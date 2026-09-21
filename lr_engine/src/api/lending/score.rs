//! Credit-score rises, paid at term end.
//!
//! The SOW (§4.1, deliverable 4) requires that gains land only when a loan
//! *term* finishes fully repaid — never mid-loan, never for depositing, never
//! for activity. The rise used to be applied inline in `repay.rs` the moment
//! the last peso landed, which meant a 3-month loan repaid on day one earned
//! exactly what one carried to maturity did. That is farmable: churn small
//! loans, climb the bands, borrow big.
//!
//! Moving the check inline would have been worse than leaving it. The score
//! code only runs when a payment arrives, so an early repayer would fail a
//! "has the term elapsed?" test at their final payment and then never be
//! looked at again — early repayment would silently forfeit the rise forever.
//! Nor could the condition go on `fully_paid` itself: that flag also closes
//! the loan, releases the deposit lots and the guarantors, and queues the
//! on-chain collateral release. Gating it would strand an early repayer's XLM
//! in the vault until their original maturity date.
//!
//! So the award leaves the repayment path entirely. `repay.rs` schedules a
//! date (`loans.score_rise_at`), and this sweep pays it when the date arrives.
//! Paying early neither forfeits the rise nor brings it forward.
//!
//! The scheduling rule, and the reason consecutive loans cannot overlap their
//! rises, is in migration 047.

use chrono::Utc;
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;

/// Score movement on a fully repaid loan. Kept here (not policy JSON) until
/// scoring gets its own policy slice — it's one number, and the log records
/// every application of it.
pub const SCORE_BUMP_ON_CLOSE: i16 = 5;

/// The ceiling from the SOW's 50–150 band.
const SCORE_MAX: i16 = 150;

/// A term end is a date, so there is nothing to gain from a tight loop. Short
/// enough that a reviewer watching the evidence run does not sit waiting.
const SWEEP_INTERVAL: Duration = Duration::from_secs(15 * 60);

/// How many loans one tick will settle. A sprint-sized pool never approaches
/// this; the cap is here so a backlog cannot hold a transaction open for an
/// unbounded time.
const BATCH: i64 = 500;

pub fn spawn_term_end_scores(pool: PgPool) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(SWEEP_INTERVAL);
        loop {
            ticker.tick().await;
            if let Err(e) = award_due(&pool, Utc::now().timestamp()).await {
                tracing::error!("term-end score sweep failed: {e}");
            }
        }
    });
}

/// Pays every rise that has come due. One transaction per loan rather than one
/// for the batch: these are independent awards to different borrowers, and a
/// single bad row should not roll back everyone else's.
async fn award_due(pool: &PgPool, now: i64) -> Result<(), sqlx::Error> {
    let due: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT id, borrower_id
           FROM public.loans
          WHERE score_rise_at IS NOT NULL
            AND score_awarded_at IS NULL
            AND score_rise_at <= $1
            AND status = 'closed'
          ORDER BY score_rise_at
          LIMIT $2",
    )
    .bind(now)
    .bind(BATCH)
    .fetch_all(pool)
    .await?;

    for (loan_id, borrower_id) in due {
        if let Err(e) = award_one(pool, loan_id, borrower_id, now).await {
            // Logged and skipped, not retried in place: the next tick will
            // find it again, since nothing was marked.
            tracing::error!(%loan_id, "term-end score award failed: {e}");
        }
    }

    Ok(())
}

async fn award_one(
    pool: &PgPool,
    loan_id: Uuid,
    borrower_id: Uuid,
    now: i64,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    // Claim the loan first, and only if it is still unpaid and still due. Two
    // sweeps overlapping — or a sweep racing a manual replay — settle exactly
    // one of them, because the second update matches no rows.
    let claimed = sqlx::query(
        "UPDATE public.loans
            SET score_awarded_at = $1
          WHERE id = $2
            AND score_awarded_at IS NULL
            AND score_rise_at IS NOT NULL
            AND score_rise_at <= $1
            AND status = 'closed'",
    )
    .bind(now)
    .bind(loan_id)
    .execute(&mut *tx)
    .await?;
    if claimed.rows_affected() == 0 {
        tx.rollback().await?;
        return Ok(());
    }

    let old_score: Option<i16> =
        sqlx::query_scalar("SELECT score FROM public.credit_scores WHERE user_id = $1 FOR UPDATE")
            .bind(borrower_id)
            .fetch_optional(&mut *tx)
            .await?;

    // A borrower with no score row has nothing to raise. The claim above still
    // stands, so the sweep does not come back to this loan every 15 minutes
    // forever.
    if let Some(old_score) = old_score {
        let new_score = (old_score + SCORE_BUMP_ON_CLOSE).min(SCORE_MAX);
        if new_score != old_score {
            sqlx::query(
                "UPDATE public.credit_scores SET score = $1, updated_at = $2 WHERE user_id = $3",
            )
            .bind(new_score)
            .bind(now)
            .bind(borrower_id)
            .execute(&mut *tx)
            .await?;

            // Same log shape the default penalty and the reconciliation refund
            // write, so the whole score history reads as one ledger.
            sqlx::query(
                "INSERT INTO public.credit_score_log (user_id, old_score, new_score, actor_id, reason)
                 VALUES ($1, $2, $3, NULL, $4)",
            )
            .bind(borrower_id)
            .bind(old_score)
            .bind(new_score)
            .bind(format!("loan {loan_id} repaid in full, term completed"))
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;
    tracing::info!(%loan_id, %borrower_id, "term-end score rise awarded");
    Ok(())
}
