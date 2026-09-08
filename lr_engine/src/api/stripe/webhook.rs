//! POST /stripe/webhook — Stripe telling us something changed on its side.
//!
//! **This endpoint never credits money.** That is the design, not an
//! omission. Money-in is pull-based on both rails — the engine asks the
//! provider what was paid when the member says they paid — because a single
//! verified read is easier to reason about than two paths that both credit and
//! must agree. A webhook that could also credit would double the number of
//! places the `rail_ref` rule has to hold.
//!
//! What it *is* for is the state Stripe changes without anyone asking:
//!
//!   * `account.updated` — a member's Connect account being cleared for
//!     payouts (or losing that clearance) happens on Stripe's timetable,
//!     sometimes days after onboarding. Polling for it would mean asking about
//!     every connected account forever.
//!   * `transfer.reversed` — a payout coming back. The worker also catches
//!     this on its next sweep; the webhook just makes it prompt.
//!
//! Authentication is the signature and nothing else: there is no session
//! cookie on this request, so the CSRF guard passes it through untouched (see
//! `infra::csrf` — it only enforces on requests that carry a session). Without
//! `STRIPE_WEBHOOK_SECRET` every post is refused rather than trusted.
//!
//! Always answers 200 once the signature checks out, even when the event is
//! one we don't handle or handling failed: a non-2xx makes Stripe retry with
//! backoff for days, and retrying won't fix an event we have no code for. The
//! failure goes to the logs instead.

use axum::{
    Extension,
    body::Bytes,
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use serde::Deserialize;
use sqlx::PgPool;

/// How stale a signature may be. Stripe's own recommendation, and the thing
/// that stops a captured request being replayed indefinitely.
const TOLERANCE_SECS: i64 = 300;

#[derive(Deserialize)]
struct Event {
    #[serde(default)]
    id: String,
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    data: EventData,
}

#[derive(Deserialize, Default)]
struct EventData {
    #[serde(default)]
    object: EventObject,
}

/// Only the fields the two handled events need. Everything else Stripe sends
/// is ignored by construction rather than by filtering.
#[derive(Deserialize, Default)]
struct EventObject {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    payouts_enabled: Option<bool>,
    #[serde(default)]
    details_submitted: Option<bool>,
    /// checkout.session.completed: the member the session was created for.
    #[serde(default)]
    client_reference_id: Option<String>,
    #[serde(default)]
    metadata: SessionMetadata,
}

/// What `create_checkout_session` stamped on, so this endpoint can tell a pool
/// deposit from a loan repayment. A session without it predates the stamping
/// and is left to the redirect rather than guessed at.
#[derive(Deserialize, Default)]
struct SessionMetadata {
    #[serde(default)]
    purpose: Option<String>,
}

pub async fn handle(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    // Bytes, not Json: the signature covers the raw body Stripe sent, and
    // re-serializing parsed JSON produces different bytes that will never
    // verify. This extractor must stay as it is.
    body: Bytes,
) -> StatusCode {
    let Some(secret) = crate::infra::stripe::webhook_secret() else {
        tracing::warn!("stripe webhook received but STRIPE_WEBHOOK_SECRET is not set — refused");
        return StatusCode::SERVICE_UNAVAILABLE;
    };

    let signature = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();

    if !crate::infra::stripe::verify_webhook(
        &body,
        signature,
        &secret,
        Utc::now().timestamp(),
        TOLERANCE_SECS,
    ) {
        tracing::warn!("stripe webhook signature rejected");
        return StatusCode::UNAUTHORIZED;
    }

    let event: Event = match serde_json::from_slice(&body) {
        Ok(event) => event,
        Err(e) => {
            tracing::error!("stripe webhook decode: {e}");
            // Signed by us but unreadable — retrying won't help.
            return StatusCode::OK;
        }
    };

    if let Err(e) = dispatch(&pool, &event).await {
        // Logged, not retried: see the module note on why this answers 200.
        tracing::error!(event = %event.id, kind = %event.kind, "stripe webhook handling failed: {e}");
    }

    StatusCode::OK
}

async fn dispatch(pool: &PgPool, event: &Event) -> Result<(), sqlx::Error> {
    match event.kind.as_str() {
        // The member paid. Normally the redirect back to the app confirms it
        // moments later — but a member who closes the tab, loses signal, or
        // never returns has money at Stripe and nothing in the pool, and the
        // only way back used to be an operator pasting the session id into a
        // URL by hand.
        //
        // This does not weaken the "one credit path" rule the module note
        // describes so much as it reroutes it: both the redirect and this
        // event land in `deposit::credit`, and both get there through
        // `capture_session`, which re-reads the session from Stripe and
        // re-checks paid/currency/owner. The event is a prompt to go and look,
        // never itself the evidence. Two prompts, one verified read, and the
        // ledger's unique rail_ref decides which one credits.
        "checkout.session.completed" => {
            let Some(session_id) = event.data.object.id.as_deref() else {
                return Ok(());
            };
            let Some(user) = event.data.object.client_reference_id.as_deref() else {
                tracing::warn!(session_id, "stripe session completed with no member on it");
                return Ok(());
            };
            let purpose = event.data.object.metadata.purpose.as_deref().unwrap_or("");
            // A repayment has to be applied against a schedule, not dropped
            // into the pool. Until that path is wired here, saying so is the
            // only safe thing: crediting it as a deposit would put a
            // borrower's payment in their savings and leave the loan open.
            if purpose != "deposit" {
                tracing::info!(session_id, purpose, "stripe session not a deposit — left to the redirect");
                return Ok(());
            }
            let Ok(user_id) = user.parse::<uuid::Uuid>() else {
                tracing::warn!(session_id, user, "stripe session carries an unreadable member id");
                return Ok(());
            };

            match crate::infra::stripe::capture_session(session_id, user).await {
                Ok(captured) => {
                    match crate::api::lending::credit_deposit(pool, user_id, "stripe", captured).await {
                        Ok(done) => {
                            tracing::info!(%user_id, session_id, amount = done.amount, "deposit credited from webhook");
                        }
                        // The redirect got there first. That is the system
                        // working, not a failure — the rail_ref wall is
                        // exactly what makes two prompts safe.
                        Err((status, message)) if status == StatusCode::CONFLICT => {
                            tracing::info!(session_id, message, "deposit already credited");
                        }
                        Err((status, message)) => {
                            tracing::error!(session_id, %status, message, "webhook deposit credit failed");
                        }
                    }
                }
                Err(message) => {
                    tracing::error!(session_id, message, "webhook could not verify the session");
                }
            }
        }
        "account.updated" => {
            let Some(account_id) = event.data.object.id.as_deref() else {
                return Ok(());
            };
            let payouts_enabled = event.data.object.payouts_enabled.unwrap_or(false);
            let details_submitted = event.data.object.details_submitted.unwrap_or(false);
            let email = event.data.object.email.clone().unwrap_or_default();

            // Scoped to a row we already know about. An account.updated for an
            // account this engine never created is not ours to act on.
            let updated = sqlx::query(
                "UPDATE public.stripe_accounts
                    SET email = CASE WHEN $1 = '' THEN email ELSE $1 END,
                        payouts_enabled = $2,
                        details_submitted = $3,
                        updated_at = $4
                  WHERE account_id = $5",
            )
            .bind(&email)
            .bind(payouts_enabled)
            .bind(details_submitted)
            .bind(Utc::now().timestamp())
            .bind(account_id)
            .execute(pool)
            .await?;

            if updated.rows_affected() > 0 {
                tracing::info!(account_id, payouts_enabled, "stripe account updated");
            }
        }
        "transfer.reversed" => {
            let Some(transfer_id) = event.data.object.id.as_deref() else {
                return Ok(());
            };
            // The money is ours again and the member is still owed it, so the
            // payable stays exactly where it is — the same reading
            // `infra::payouts` applies to a returned PayPal payout. They can
            // ask again.
            //
            // Only a payout that hasn't settled is moved: a reversal arriving
            // after `paid` needs a human, because the books have already been
            // posted and unposting them is not a thing a webhook should do.
            let updated = sqlx::query(
                "UPDATE public.payouts
                    SET status = 'returned',
                        last_error = 'The transfer was reversed'
                  WHERE provider = 'stripe'
                    AND batch_id = $1
                    AND status IN ('sent', 'unclaimed')",
            )
            .bind(transfer_id)
            .execute(pool)
            .await?;

            if updated.rows_affected() > 0 {
                tracing::warn!(transfer_id, "stripe transfer reversed");
            } else {
                tracing::warn!(transfer_id, "stripe transfer reversed after settlement — needs review");
            }
        }
        // Everything else is Stripe keeping us informed about things the
        // engine reads directly when it needs them.
        _ => {}
    }
    Ok(())
}
