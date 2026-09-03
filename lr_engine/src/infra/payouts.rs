//! The payout worker: retries what PayPal never received, and reconciles what
//! it did.
//!
//! Two jobs, on one tick:
//!
//!   * **submit** rows still `pending` — a payout whose HTTP call timed out or
//!     whose process died mid-request. The retry carries the SAME
//!     `sender_batch_id` (the row's primary key), so PayPal refuses a
//!     duplicate rather than sending twice.
//!   * **reconcile** rows already `sent` or `unclaimed`, by asking PayPal what
//!     happened to the batch.
//!
//! The books move here and nowhere else. `disburse` raised `payout_payable`;
//! only a PayPal-confirmed SUCCESS pays it down against `cash`. That posting
//! carries the transfer's own reference as the ledger's unique `rail_ref`, so
//! even if this worker ran twice over the same payout, the second posting
//! bounces off the schema — the same idempotency rule money-in has always had.
//!
//! A payout that comes back (returned, reversed, refused after acceptance)
//! restores the promise rather than cancelling it: the member is still owed
//! their proceeds, so the payable stays and the row goes terminal so they can
//! request it again.

use std::time::Duration;

use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::lending::ledger::{EventDraft, Posting, commit_event};
use crate::infra::paypal::{self, PayoutOutcome, SubmitError};

const TICK: Duration = Duration::from_secs(60);
/// Don't re-submit a row the request handler is still working on.
const SUBMIT_AFTER_SECS: i64 = 90;
/// Give up re-submitting after this many tries; the row stays `pending` and
/// visible rather than being silently abandoned.
const MAX_ATTEMPTS: i32 = 8;
/// Rows examined per tick. Small on purpose — this is money, and a slow
/// drain is better than a burst against a rate-limited provider.
const BATCH: i64 = 20;

/// Spawns the worker. Does nothing at all when PayPal isn't configured, so a
/// deployment without credentials doesn't log an error every minute.
pub fn spawn(pool: PgPool) {
    if !paypal::is_configured() {
        tracing::info!("payout worker not started — PayPal is not configured");
        return;
    }
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(TICK);
        loop {
            ticker.tick().await;
            if let Err(e) = submit_pending(&pool).await {
                tracing::error!("payout submit sweep: {e}");
            }
            if let Err(e) = reconcile_sent(&pool).await {
                tracing::error!("payout reconcile sweep: {e}");
            }
        }
    });
}

type PendingRow = (Uuid, Uuid, i64, String, Option<Uuid>, i32);

/// The ledger event a settled payout is filed under. Both kinds pay down the
/// same `payout_payable`, but the reason the pool owed the money is worth
/// keeping in the event stream — a withdrawal is not a loan.
fn settled_event_kind(kind: &str) -> &'static str {
    match kind {
        "deposit_withdrawal" => "withdrawal_paid",
        _ => "loan_payout_paid",
    }
}

async fn submit_pending(pool: &PgPool) -> Result<(), sqlx::Error> {
    let cutoff = Utc::now().timestamp() - SUBMIT_AFTER_SECS;
    let rows: Vec<PendingRow> = sqlx::query_as(
        "SELECT id, user_id, amount, payer_id, loan_id, attempts
           FROM public.payouts
          WHERE status = 'pending' AND created_at <= $1 AND attempts < $2
          ORDER BY created_at
          LIMIT $3",
    )
    .bind(cutoff)
    .bind(MAX_ATTEMPTS)
    .bind(BATCH)
    .fetch_all(pool)
    .await?;

    for (id, user_id, amount, payer_id, loan_id, attempts) in rows {
        // Same wording the request handlers used on the first attempt, so a
        // retry doesn't show the recipient a different transfer.
        let note = match loan_id {
            Some(loan_id) => format!("PrimeLendRow loan {loan_id}"),
            None => "PrimeLendRow withdrawal".to_string(),
        };
        // Same id every time: this is a retry of ONE payout, not a new one.
        let result = paypal::create_payout(&id.to_string(), &payer_id, amount, &note).await;
        let now = Utc::now().timestamp();

        let (status, batch_id, sent_at, error) = match result {
            Ok(batch_id) => {
                tracing::info!(%user_id, payout = %id, attempt = attempts + 1, "payout submitted on retry");
                ("sent", Some(batch_id), Some(now), None)
            }
            Err(SubmitError::AlreadySubmitted) => (
                "sent",
                None,
                Some(now),
                Some("PayPal already had this payout — reconciling".to_string()),
            ),
            Err(SubmitError::Refused(reason)) => ("failed", None, None, Some(reason)),
            Err(SubmitError::Retryable(reason)) => ("pending", None, None, Some(reason)),
        };

        sqlx::query(
            "UPDATE public.payouts
                SET status = $1,
                    batch_id = COALESCE($2, batch_id),
                    sent_at = COALESCE($3, sent_at),
                    last_error = $4,
                    attempts = attempts + 1
              WHERE id = $5 AND status = 'pending'",
        )
        .bind(status)
        .bind(batch_id)
        .bind(sent_at)
        .bind(error.map(|e| e.chars().take(200).collect::<String>()))
        .bind(id)
        .execute(pool)
        .await?;
    }
    Ok(())
}

async fn reconcile_sent(pool: &PgPool) -> Result<(), sqlx::Error> {
    let rows: Vec<(Uuid, Uuid, i64, Option<Uuid>, String, String)> = sqlx::query_as(
        "SELECT id, user_id, amount, loan_id, batch_id, kind
           FROM public.payouts
          WHERE status IN ('sent', 'unclaimed') AND batch_id IS NOT NULL
          ORDER BY sent_at
          LIMIT $1",
    )
    .bind(BATCH)
    .fetch_all(pool)
    .await?;

    for (id, user_id, amount, loan_id, batch_id, kind) in rows {
        let outcome = match paypal::payout_status(&batch_id).await {
            Ok(outcome) => outcome,
            Err(reason) => {
                tracing::warn!(payout = %id, reason, "payout status unavailable");
                continue;
            }
        };

        match outcome {
            PayoutOutcome::Paid { item_id, transaction_id } => {
                settle(
                    pool,
                    id,
                    user_id,
                    amount,
                    loan_id,
                    settled_event_kind(&kind),
                    &item_id,
                    transaction_id,
                )
                .await?;
            }
            PayoutOutcome::Unclaimed { item_id } => {
                set_status(pool, id, "unclaimed", item_id, None).await?;
            }
            PayoutOutcome::Returned { item_id, reason } => {
                // The money is ours again and the member is still owed it, so
                // the payable stays exactly where it is. They can ask again.
                tracing::warn!(payout = %id, reason, "payout returned");
                set_status(pool, id, "returned", item_id, Some(reason)).await?;
            }
            PayoutOutcome::Failed { reason } => {
                tracing::warn!(payout = %id, reason, "payout failed");
                set_status(pool, id, "failed", None, Some(reason)).await?;
            }
            PayoutOutcome::Pending { .. } => {}
        }
    }
    Ok(())
}

async fn set_status(
    pool: &PgPool,
    id: Uuid,
    status: &str,
    item_id: Option<String>,
    error: Option<String>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE public.payouts
            SET status = $1,
                item_id = COALESCE($2, item_id),
                last_error = COALESCE($3, last_error)
          WHERE id = $4",
    )
    .bind(status)
    .bind(item_id)
    .bind(error.map(|e| e.chars().take(200).collect::<String>()))
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// The only place a payout moves the books.
#[allow(clippy::too_many_arguments)]
async fn settle(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
    amount: i64,
    loan_id: Option<Uuid>,
    event_kind: &'static str,
    item_id: &str,
    transaction_id: Option<String>,
) -> Result<(), sqlx::Error> {
    let now = Utc::now().timestamp();
    let mut tx = pool.begin().await?;

    // Claim the row first: `status = 'paid'` under the same transaction as
    // the posting means two workers racing produce one winner, and the
    // loser's UPDATE matches nothing.
    let claimed = sqlx::query(
        "UPDATE public.payouts
            SET status = 'paid', item_id = $1, transaction_id = $2, settled_at = $3
          WHERE id = $4 AND status IN ('sent', 'unclaimed')",
    )
    .bind(item_id)
    .bind(&transaction_id)
    .bind(now)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    if claimed.rows_affected() == 0 {
        tx.rollback().await?;
        return Ok(());
    }

    // The transfer's own reference is the rail_ref, so this credit can only
    // ever be posted once — the same wall a re-sent PayPal capture hits.
    let rail_ref = format!(
        "paypal_payout:{}",
        transaction_id.as_deref().unwrap_or(item_id)
    );
    let posted = commit_event(
        &mut tx,
        EventDraft {
            kind: event_kind,
            user_id: Some(user_id),
            loan_id,
            deposit_id: None,
            rail_ref: Some(rail_ref),
            payload: serde_json::json!({
                "payout_id": id, "amount": amount, "item_id": item_id,
                "transaction_id": transaction_id, "rail": "paypal_payouts",
            }),
            actor_id: None,
        },
        // The promise is settled and the pesos really have left now.
        &[
            Posting { account: "payout_payable", amount },
            Posting { account: "cash", amount: -amount },
        ],
    )
    .await;

    match posted {
        Ok(_) => {
            tx.commit().await?;
            tracing::info!(%user_id, payout = %id, amount, "payout settled and posted");
        }
        Err(crate::api::lending::ledger::LedgerError::DuplicateRail) => {
            // Already posted by an earlier run; the status claim above is the
            // only thing that needed catching up, so keep it.
            tx.commit().await?;
            tracing::info!(payout = %id, "payout already posted — status reconciled");
        }
        Err(e) => {
            tx.rollback().await?;
            let detail = match e {
                crate::api::lending::ledger::LedgerError::Unbalanced(net) => {
                    format!("unbalanced by {net}")
                }
                crate::api::lending::ledger::LedgerError::Db(e) => e.to_string(),
                crate::api::lending::ledger::LedgerError::DuplicateRail => unreachable!(),
            };
            tracing::error!(payout = %id, detail, "payout posting failed — will retry");
        }
    }
    Ok(())
}
