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
use crate::infra::rails::{PayoutOutcome, SubmitError};
use crate::infra::{paypal, stripe};

const TICK: Duration = Duration::from_secs(60);
/// Don't re-submit a row the request handler is still working on.
const SUBMIT_AFTER_SECS: i64 = 90;
/// Give up re-submitting after this many tries; the row stays `pending` and
/// visible rather than being silently abandoned.
const MAX_ATTEMPTS: i32 = 8;
/// Rows examined per tick. Small on purpose — this is money, and a slow
/// drain is better than a burst against a rate-limited provider.
const BATCH: i64 = 20;

/// Spawns the worker. Does nothing at all when neither rail is configured, so
/// a deployment without credentials doesn't log an error every minute. One
/// rail is enough: rows belonging to the unconfigured one simply fail their
/// own `is_configured` check and stay visible as `pending`.
pub fn spawn(pool: PgPool) {
    if !paypal::is_configured() && !stripe::is_configured() {
        tracing::info!("payout worker not started — no payment rail is configured");
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
            if let Err(e) = refund_stranded_withdrawals(&pool).await {
                tracing::error!("withdrawal refund sweep: {e}");
            }
        }
    });
}

type PendingRow = (Uuid, Uuid, i64, String, Option<Uuid>, i32, String);
/// id, user_id, amount, loan_id, batch_id, kind, provider — in SELECT order.
type SentRow = (Uuid, Uuid, i64, Option<Uuid>, String, String, String);

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
        "SELECT id, user_id, amount, payer_id, loan_id, attempts, provider
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

    for (id, user_id, amount, payer_id, loan_id, attempts, provider) in rows {
        // A rail this deployment has no credentials for can't be retried, and
        // burning an attempt on it would quietly exhaust MAX_ATTEMPTS and
        // abandon the row. Skip instead: it stays pending and visible.
        if !rail_configured(&provider) {
            continue;
        }
        // Same wording the request handlers used on the first attempt, so a
        // retry doesn't show the recipient a different transfer.
        let note = match loan_id {
            Some(loan_id) => format!("PrimeLendRow loan {loan_id}"),
            None => "PrimeLendRow withdrawal".to_string(),
        };
        // Same id every time: this is a retry of ONE payout, not a new one.
        // Both providers refuse a duplicate under that id, which is the whole
        // reason a retry here is safe.
        let result =
            crate::api::lending::submit_payout(&provider, id, &payer_id, amount, &note).await;
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
                Some("The provider already had this payout — reconciling".to_string()),
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

        // A withdrawal that ran out of road goes back into the member's
        // deposits. No-op for loan proceeds, and for anything not terminal.
        if status == "failed" {
            crate::api::lending::refund_failed_withdrawal(pool, id).await?;
        }
    }
    Ok(())
}

/// Gives back every withdrawal that died without its money being returned.
///
/// The two sweeps above only look at rows still in motion (`pending`, `sent`,
/// `unclaimed`), so a withdrawal that reached `failed` or `returned` is seen by
/// neither. That leaves two ways for a member's deposit to go missing, and this
/// closes both:
///
///   * rows that failed **before this refund existed** — the money was consumed
///     into a payable that nothing was ever going to pay down;
///   * rows where the refund itself couldn't be written at the time (the
///     database was briefly unavailable, the process died between marking the
///     row and reversing the postings).
///
/// The `NOT EXISTS` is the whole guard, and it reads the same fact the refund
/// writes: the ledger event's unique `rail_ref`. So this can run every minute
/// forever, find nothing, and cost one indexed lookup — and if it ever does
/// find something, `refund_if_failed` is itself idempotent, so a race with the
/// request handler still refunds exactly once.
///
/// This is the money-in mirror of what the payout retry already does for money
/// out: no member's balance is left depending on a single request going well.
async fn refund_stranded_withdrawals(pool: &PgPool) -> Result<(), sqlx::Error> {
    let ids: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT p.id
           FROM public.payouts p
          WHERE p.kind = 'deposit_withdrawal'
            AND p.status IN ('failed', 'returned')
            AND NOT EXISTS (
                SELECT 1 FROM public.ledger_events e
                 WHERE e.rail_ref = 'withdrawal_refund:' || p.id::text
            )
          ORDER BY p.created_at
          LIMIT $1",
    )
    .bind(BATCH)
    .fetch_all(pool)
    .await?;

    for (id,) in ids {
        tracing::warn!(payout = %id, "stranded withdrawal found — refunding to deposits");
        crate::api::lending::refund_failed_withdrawal(pool, id).await?;
    }
    Ok(())
}

/// Can this deployment talk to the rail a row was submitted over? Checked per
/// row rather than once, because a deployment may run one rail and still hold
/// history from the other.
fn rail_configured(provider: &str) -> bool {
    match provider {
        "stripe" => stripe::is_configured(),
        _ => paypal::is_configured(),
    }
}

/// The latest word on a submitted transfer, from whichever provider sent it.
async fn rail_status(provider: &str, reference: &str) -> Result<PayoutOutcome, &'static str> {
    match provider {
        "stripe" => stripe::transfer_status(reference).await,
        _ => paypal::payout_status(reference).await,
    }
}

async fn reconcile_sent(pool: &PgPool) -> Result<(), sqlx::Error> {
    let rows: Vec<SentRow> = sqlx::query_as(
        "SELECT id, user_id, amount, loan_id, batch_id, kind, provider
           FROM public.payouts
          WHERE status IN ('sent', 'unclaimed') AND batch_id IS NOT NULL
          ORDER BY sent_at
          LIMIT $1",
    )
    .bind(BATCH)
    .fetch_all(pool)
    .await?;

    for (id, user_id, amount, loan_id, batch_id, kind, provider) in rows {
        if !rail_configured(&provider) {
            continue;
        }
        let outcome = match rail_status(&provider, &batch_id).await {
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
                    &provider,
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
                // the payable stays exactly where it is — for loan proceeds,
                // which they can ask for again. A withdrawal has nothing left
                // to ask with (its lots were consumed at request time), so
                // `refund_failed_withdrawal` gives that one its deposit back.
                tracing::warn!(payout = %id, reason, "payout returned");
                set_status(pool, id, "returned", item_id, Some(reason)).await?;
                crate::api::lending::refund_failed_withdrawal(pool, id).await?;
            }
            PayoutOutcome::Failed { reason } => {
                tracing::warn!(payout = %id, reason, "payout failed");
                set_status(pool, id, "failed", None, Some(reason)).await?;
                crate::api::lending::refund_failed_withdrawal(pool, id).await?;
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
    provider: &str,
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
    // ever be posted once — the same wall a re-sent capture hits.
    //
    // Namespaced by provider: the two issue reference ids from separate spaces
    // and a bare id from one could in principle collide with the other's,
    // which on a UNIQUE index would silently refuse a legitimate settlement.
    let rail_ref = format!(
        "{provider}_payout:{}",
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
                "transaction_id": transaction_id, "rail": provider,
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
