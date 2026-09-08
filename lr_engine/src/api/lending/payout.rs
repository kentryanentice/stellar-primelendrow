//! POST /loans/payout — send a disbursed loan's proceeds to the borrower's
//! own PayPal.
//! GET  /payouts      — the caller's transfers and where each one got to.
//!
//! Disbursement makes the pool *owe* the borrower (`payout_payable`, 028);
//! this is where that promise is actually settled. `withdraw.rs` settles the
//! depositor's equivalent promise through the same three steps below (029) —
//! `submit` and `read_one` are shared with it rather than reimplemented, so
//! there is one description of "hand money to PayPal" in the engine.
//!
//! The order of operations is the entire safety argument, and it is the same
//! shape as the collateral outbox:
//!
//!   1. write the payout row, and COMMIT it — its primary key is the
//!      idempotency key PayPal will be given, so the key must exist before
//!      any money can;
//!   2. call PayPal outside any transaction, holding no locks;
//!   3. record what PayPal said.
//!
//! If step 2 times out, crashes, or is double-clicked, the retry carries the
//! same `sender_batch_id` and PayPal refuses it — there is no path that pays
//! twice. The books move only when PayPal confirms the transfer succeeded,
//! which happens in `infra::payouts`, not here: a submitted payout is not a
//! paid one.

use axum::{Extension, Json, http::{HeaderMap, StatusCode}};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::shared::db_err;
use crate::api::users::shared::{E, require_verified_user};
use crate::infra::rails::SubmitError;
use crate::infra::{paypal, stripe};

#[derive(Deserialize)]
pub struct PayoutInput {
    loan_id: Uuid,
}

#[derive(Serialize)]
pub struct PayoutView {
    pub id: Uuid,
    pub loan_id: Option<Uuid>,
    /// 'loan_proceeds' or 'deposit_withdrawal' — the two reasons money leaves,
    /// so the UI can file a transfer under the right card without guessing
    /// from a null loan_id.
    pub kind: String,
    /// Which rail carried it, 'paypal' or 'stripe'. Published so the UI can
    /// name the member's own account rather than whichever provider the
    /// wording happened to be written against.
    pub provider: String,
    pub amount: i64,
    pub status: String,
    /// PayPal's own reference, once there is one to look up.
    pub batch_id: Option<String>,
    pub transaction_id: Option<String>,
    pub created_at: i64,
    pub settled_at: Option<i64>,
    /// Why a payout is stuck or came back, in the member's language.
    pub note: Option<String>,
}

#[derive(Serialize)]
pub struct PayoutResponse {
    pub payout: PayoutView,
    pub message: &'static str,
}

/// Where a member's money goes, and over which rail.
///
/// Both are pinned to the payout row at request time. Disconnecting or
/// relinking an account afterwards must not redirect a transfer already in
/// flight, so no later code path resolves a destination for a payout that
/// already exists — the worker reads `provider` and `payer_id` off the row.
pub(super) struct Destination {
    /// "paypal" or "stripe".
    pub provider: &'static str,
    /// The provider's own reference for the member: a PayPal payer id, or a
    /// Stripe connected account id.
    pub account: String,
}

/// Which rail this deployment prefers when a member has connected both.
///
/// Defaults to PayPal, which is the rail every existing member is already on —
/// so installing the Stripe code changes nobody's destination until an
/// operator says so. Set `PAYOUT_RAIL=stripe` to flip the preference; either
/// way the other rail remains a fallback, because a member who has only
/// connected one should be payable over that one.
fn preferred_rail() -> &'static str {
    match std::env::var("PAYOUT_RAIL").as_deref() {
        Ok("stripe") => "stripe",
        _ => "paypal",
    }
}

/// Resolves the member's payout destination.
///
/// Shared with `withdraw.rs`: both money-out paths need exactly this check, in
/// this order — a deployment with no usable rail fails before a row is written
/// rather than leaving a promise nothing can ever send.
///
/// The two rails are not checked symmetrically, because they don't mean the
/// same thing. A PayPal account is payable as soon as it is linked. A Stripe
/// connected account is only payable once Stripe says `payouts_enabled` — an
/// abandoned onboarding leaves a row that looks connected and would refuse
/// every transfer, so it is filtered out here rather than discovered later by
/// the worker.
pub(super) async fn destination(pool: &PgPool, user_id: Uuid) -> Result<Destination, E> {
    let paypal_account: Option<String> = if paypal::is_configured() {
        sqlx::query_scalar(
            "SELECT payer_id FROM public.paypal_accounts
              WHERE user_id = $1 AND status = 'active'",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| db_err(e, "paypal account"))?
    } else {
        None
    };

    let stripe_account: Option<String> = if stripe::is_configured() {
        sqlx::query_scalar(
            "SELECT account_id FROM public.stripe_accounts
              WHERE user_id = $1 AND status = 'active' AND payouts_enabled",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| db_err(e, "stripe account"))?
    } else {
        None
    };

    let (first, second) = match preferred_rail() {
        "stripe" => (
            stripe_account.map(|a| Destination { provider: "stripe", account: a }),
            paypal_account.map(|a| Destination { provider: "paypal", account: a }),
        ),
        _ => (
            paypal_account.map(|a| Destination { provider: "paypal", account: a }),
            stripe_account.map(|a| Destination { provider: "stripe", account: a }),
        ),
    };

    if let Some(destination) = first.or(second) {
        return Ok(destination);
    }

    // Nothing to pay to. Which of the two failures it is decides what the
    // member should do about it, so they are not collapsed into one message.
    if !paypal::is_configured() && !stripe::is_configured() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Payouts aren't configured on this deployment yet",
        ));
    }
    Err((
        StatusCode::UNPROCESSABLE_ENTITY,
        "Connect a payout account first — Settings → PayPal or Stripe",
    ))
}

pub async fn request(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(p): Json<PayoutInput>,
) -> Result<Json<PayoutResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;
    let destination = destination(&pool, user_id).await?;

    let mut tx = pool.begin().await.map_err(|e| db_err(e, "begin payout"))?;

    // The loan row is the serialization point against a second click.
    let loan: Option<(Uuid, i64, String)> = sqlx::query_as(
        "SELECT borrower_id, principal, status FROM public.loans
          WHERE id = $1 FOR UPDATE",
    )
    .bind(p.loan_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| db_err(e, "lock loan"))?;
    let (borrower_id, principal, status) =
        loan.ok_or((StatusCode::NOT_FOUND, "No such loan"))?;
    if borrower_id != user_id {
        return Err((StatusCode::NOT_FOUND, "No such loan"));
    }
    // Only a disbursed loan has proceeds to send. A pending one hasn't been
    // funded; a closed one was settled long ago.
    if status != "active" {
        return Err((
            StatusCode::CONFLICT,
            "This loan has no proceeds waiting — only a disbursed loan can be withdrawn",
        ));
    }

    let payout_id: Uuid = sqlx::query_scalar(
        "INSERT INTO public.payouts (user_id, loan_id, kind, amount, payer_id, provider)
         VALUES ($1, $2, 'loan_proceeds', $3, $4, $5)
         RETURNING id",
    )
    .bind(user_id)
    .bind(p.loan_id)
    .bind(principal)
    .bind(&destination.account)
    .bind(destination.provider)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        if e.as_database_error().is_some_and(|d| d.is_unique_violation()) {
            // idx_payouts_one_per_loan: these proceeds are already on the way.
            return (
                StatusCode::CONFLICT,
                "These proceeds have already been sent to your PayPal",
            );
        }
        db_err(e, "insert payout")
    })?;

    // Committed BEFORE the network call: the idempotency key has to survive a
    // crash that happens mid-request, or a retry would mint a new one and
    // PayPal would have no way to recognise the duplicate.
    tx.commit().await.map_err(|e| db_err(e, "commit payout"))?;

    let (status, message) = submit(
        &pool,
        payout_id,
        &destination,
        principal,
        &format!("PrimeLendRow loan {}", p.loan_id),
    )
    .await;
    let payout = read_one(&pool, payout_id, user_id).await?;
    tracing::info!(%user_id, %payout_id, principal, status, "loan payout requested");

    Ok(Json(PayoutResponse { payout, message }))
}

/// Hands the payout to its provider, whichever that is.
///
/// The dispatch lives here and in `infra::payouts` and nowhere else — one
/// place for a request-time submission, one for a retry — so a third rail
/// would add two match arms and touch no handler.
///
/// Both providers key the submission on the payout id, and both refuse a
/// duplicate: PayPal because it has seen the `sender_batch_id`, Stripe because
/// `create_transfer` looks the `transfer_group` up before it posts. That is
/// what makes this safe to call again after a timeout.
pub(crate) async fn submit_to(
    provider: &str,
    payout_id: Uuid,
    account: &str,
    amount: i64,
    note: &str,
) -> Result<String, SubmitError> {
    let key = payout_id.to_string();
    match provider {
        "stripe" => stripe::create_transfer(&key, account, amount, note).await,
        _ => paypal::create_payout(&key, account, amount, note).await,
    }
}

/// Submits and records the answer. Never returns an error: a submission that
/// couldn't be made is a row left `pending` for the worker to retry, not a
/// failed request — the member's money claim already exists.
///
/// `note` is what the recipient sees on the transfer; the caller supplies it
/// because only the caller knows why the money is moving.
///
/// The member-facing wording deliberately says "your account" rather than
/// naming the provider: which rail carried it is an operational detail, and a
/// member who connected Stripe should not be told about PayPal.
pub(super) async fn submit(
    pool: &PgPool,
    payout_id: Uuid,
    destination: &Destination,
    amount: i64,
    note: &str,
) -> (&'static str, &'static str) {
    let now = Utc::now().timestamp();

    match submit_to(destination.provider, payout_id, &destination.account, amount, note).await {
        Ok(reference) => {
            mark(pool, payout_id, "sent", Some(&reference), Some(now), None).await;
            ("sent", "Sent — it usually lands in a few minutes")
        }
        Err(SubmitError::AlreadySubmitted) => {
            // The provider has seen this key, so a transfer may already exist.
            // Marking it sent (not retrying) is the safe reading; the worker
            // will pick up the real status, and the books stay unposted until
            // it does.
            mark(
                pool,
                payout_id,
                "sent",
                None,
                Some(now),
                Some("Already submitted — reconciling"),
            )
            .await;
            ("sent", "Already sent — checking on it")
        }
        Err(SubmitError::Refused(reason)) => {
            mark(pool, payout_id, "failed", None, None, Some(&reason)).await;
            ("failed", "The transfer was refused — check your connected account")
        }
        Err(SubmitError::Retryable(reason)) => {
            mark(pool, payout_id, "pending", None, None, Some(&reason)).await;
            ("pending", "Queued — the payment provider was unreachable, we'll keep trying")
        }
    }
}

async fn mark(
    pool: &PgPool,
    payout_id: Uuid,
    status: &str,
    batch_id: Option<&str>,
    sent_at: Option<i64>,
    error: Option<&str>,
) {
    let truncated = error.map(|e| e.chars().take(200).collect::<String>());
    if let Err(e) = sqlx::query(
        "UPDATE public.payouts
            SET status = $1,
                batch_id = COALESCE($2, batch_id),
                sent_at = COALESCE($3, sent_at),
                last_error = $4,
                attempts = attempts + 1
          WHERE id = $5",
    )
    .bind(status)
    .bind(batch_id)
    .bind(sent_at)
    .bind(truncated)
    .bind(payout_id)
    .execute(pool)
    .await
    {
        // The row stays as it was; the worker reconciles from PayPal either
        // way, so this is a logging failure, not a money one.
        tracing::error!("DB payout mark {payout_id}: {e}");
    }
}

fn note_for(status: &str, last_error: Option<String>) -> Option<String> {
    match status {
        // Only the PayPal rail can produce `unclaimed` — a Stripe transfer has
        // nothing for a recipient to accept — so naming PayPal here is
        // accurate rather than a leftover.
        "unclaimed" => Some(
            "Sent, but not accepted yet — check the email PayPal sent you. It returns to us after 30 days."
                .to_string(),
        ),
        "returned" => Some("Came back to us — you can request it again.".to_string()),
        "failed" => last_error,
        _ => None,
    }
}

/// One payouts row in `SELECT` order, shared by the single read and the list
/// so the two can't drift into disagreeing about column positions.
type PayoutRow = (
    Uuid,
    Option<Uuid>,
    String,
    String,
    i64,
    String,
    Option<String>,
    Option<String>,
    i64,
    Option<i64>,
    Option<String>,
);

const PAYOUT_COLUMNS: &str = "id, loan_id, kind, provider, amount, status, batch_id,
                              transaction_id, created_at, settled_at, last_error";

fn view(row: PayoutRow) -> PayoutView {
    let (id, loan_id, kind, provider, amount, status, batch_id, transaction_id, created_at, settled_at, last_error) = row;
    PayoutView {
        note: note_for(&status, last_error),
        id, loan_id, kind, provider, amount, status, batch_id, transaction_id, created_at, settled_at,
    }
}

pub(super) async fn read_one(pool: &PgPool, payout_id: Uuid, user_id: Uuid) -> Result<PayoutView, E> {
    let row: PayoutRow = sqlx::query_as(&format!(
        "SELECT {PAYOUT_COLUMNS} FROM public.payouts WHERE id = $1 AND user_id = $2"
    ))
    .bind(payout_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| db_err(e, "payout read"))?;

    Ok(view(row))
}

#[derive(Serialize)]
pub struct PayoutsResponse {
    pub payouts: Vec<PayoutView>,
}

pub async fn list(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
) -> Result<Json<PayoutsResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    let rows: Vec<PayoutRow> = sqlx::query_as(&format!(
        "SELECT {PAYOUT_COLUMNS} FROM public.payouts WHERE user_id = $1
          ORDER BY created_at DESC LIMIT 50"
    ))
    .bind(user_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "payouts"))?;

    Ok(Json(PayoutsResponse {
        payouts: rows.into_iter().map(view).collect(),
    }))
}
