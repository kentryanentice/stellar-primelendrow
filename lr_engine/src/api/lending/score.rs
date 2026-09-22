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
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;
use std::time::Duration;

use super::shared::db_err;
use crate::api::users::shared::E;

/// Score movement on a fully repaid loan. Kept here (not policy JSON) until
/// scoring gets its own policy slice — it's one number, and the log records
/// every application of it.
pub const SCORE_BUMP_ON_CLOSE: i16 = 5;

/// What a guarantor loses when their pledge is actually claimed.
///
/// Charged on the claim, not on the default: a guarantor whose borrower went
/// bad but whose pledge was never touched — because the borrower's own deposit
/// and collateral covered it — has had their promise tested and not called on,
/// and pays nothing. That matches the SOW's "a guarantor whose pledge **is
/// claimed** loses score".
///
/// Much lighter than the borrower's own 25. A guarantor did not take the money
/// and did not choose to stop paying; they backed someone who did. 10 is a
/// real mark without being a tier wipe, which is what the flat shape argues
/// for — see the note on proportionality below.
///
/// Paired with `SCORE_RESTORE_ON_GUARANTOR_SETTLE`: claimed then settled costs
/// a net 5, the same net a borrower carries for a settled default.
///
/// **Flat, not proportional.** A guarantor charged a peso takes the same 10 as
/// one wiped out. That was defensible when claims drained pledges oldest-first
/// and everyone hit lost everything; now that claims are pro-rata, partial
/// claims are the ordinary case. Scaling this by the fraction of the pledge
/// actually claimed is the obvious refinement if the pilot says it bites wrong.
pub const SCORE_PENALTY_ON_CLAIM: i16 = 10;

/// Given back to a claimed guarantor when the default is later settled.
///
/// A settlement refunds the guarantor's seized deposit in full, so leaving the
/// score untouched would make them whole in pesos and permanently down on
/// record for a loan that ended up square. Most of the penalty comes back; 5
/// does not, because the claim did happen.
pub const SCORE_RESTORE_ON_GUARANTOR_SETTLE: i16 = 5;

/// The SOW's 50–150 band.
const SCORE_MAX: i16 = 150;
const SCORE_MIN: i16 = 50;

/// Stable identifiers for why a score moved (048). The UI and the public proof
/// page switch on these; `reason` carries the sentence a human reads.
pub mod reason {
    pub const REPAID_TERM_COMPLETE: &str = "loan_repaid_term_complete";
    pub const GUARANTOR_CLAIMED: &str = "guarantor_claimed";
    pub const GUARANTOR_CLAIM_SETTLED: &str = "guarantor_claim_settled";
}

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
            // write, so the whole score history reads as one ledger — now with
            // the reason as a code and the loan as a column (048).
            sqlx::query(
                "INSERT INTO public.credit_score_log
                     (user_id, old_score, new_score, actor_id, reason, reason_code, loan_id)
                 VALUES ($1, $2, $3, NULL, $4, $5, $6)",
            )
            .bind(borrower_id)
            .bind(old_score)
            .bind(new_score)
            .bind(format!("loan {loan_id} repaid in full, term completed"))
            .bind(reason::REPAID_TERM_COMPLETE)
            .bind(loan_id)
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;
    tracing::info!(%loan_id, %borrower_id, "term-end score rise awarded");
    Ok(())
}

/// Charges a guarantor for a claim against their pledge (SOW §4.1, D2 and D4).
///
/// Called from the recovery waterfall once a guarantor's own lots have actually
/// been seized — not when a claim is merely calculated. A guarantor who was
/// apportioned a share but had nothing left to take has not had a pledge
/// claimed, and is not charged for one.
///
/// Runs inside the caller's transaction on purpose: the seizure and the score
/// drop are one event, and a guarantor whose deposit was taken without the
/// penalty being recorded would be the sort of disagreement the audit log
/// exists to make impossible.
pub async fn penalise_guarantor_claim(
    tx: &mut Transaction<'_, Postgres>,
    loan_id: Uuid,
    guarantor_id: Uuid,
    actor_id: Uuid,
    now: i64,
) -> Result<(), E> {
    // Once per guarantor per loan. `recovery::advance` is resumable — it runs
    // the waterfall as far as the facts allow, stops at a locked XLM position,
    // and is called again once the chain confirms the seizure. If a shortfall
    // survives that second pass, step 3 runs a second time and can take more
    // from a guarantor who still has pledged lots. That is correct for the
    // money and wrong for the record: the pledge was claimed once.
    let already: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM public.credit_score_log
          WHERE loan_id = $1 AND user_id = $2 AND reason_code = $3)",
    )
    .bind(loan_id)
    .bind(guarantor_id)
    .bind(reason::GUARANTOR_CLAIMED)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| db_err(e, "guarantor penalty check"))?;
    if already {
        return Ok(());
    }

    let old_score: Option<i16> =
        sqlx::query_scalar("SELECT score FROM public.credit_scores WHERE user_id = $1 FOR UPDATE")
            .bind(guarantor_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(|e| db_err(e, "guarantor score read"))?;

    let Some(old_score) = old_score else {
        return Ok(());
    };
    let new_score = (old_score - SCORE_PENALTY_ON_CLAIM).max(SCORE_MIN);
    if new_score == old_score {
        return Ok(());
    }

    sqlx::query("UPDATE public.credit_scores SET score = $1, updated_at = $2 WHERE user_id = $3")
        .bind(new_score)
        .bind(now)
        .bind(guarantor_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| db_err(e, "guarantor score penalty"))?;

    sqlx::query(
        "INSERT INTO public.credit_score_log
             (user_id, old_score, new_score, actor_id, reason, reason_code, loan_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(guarantor_id)
    .bind(old_score)
    .bind(new_score)
    .bind(actor_id)
    .bind(format!("guarantor pledge claimed on loan {loan_id}"))
    .bind(reason::GUARANTOR_CLAIMED)
    .bind(loan_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| db_err(e, "guarantor score log"))?;

    tracing::info!(%loan_id, %guarantor_id, old_score, new_score, "guarantor penalised for claim");
    Ok(())
}

/// Gives back most of what a claim cost, when the default is later settled.
///
/// The settlement refunds every claimed guarantor their seized deposit in full
/// (`reconcile::settle` creates the refund lots). Their record follows the
/// money: if the claim is unwound, the reason for the penalty is unwound with
/// it, less the 5 that stays because the claim did happen.
///
/// Finds who to pay from the score log itself rather than from the recovery
/// rows — the log is the record of who was actually penalised, so nobody can
/// be restored who was never charged, and a guarantor whose penalty was
/// skipped (no score row) is not handed 5 points out of nowhere.
pub async fn restore_guarantor_claims(
    tx: &mut Transaction<'_, Postgres>,
    loan_id: Uuid,
    actor_id: Uuid,
    now: i64,
) -> Result<(), E> {
    let claimed: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT l.user_id
           FROM public.credit_score_log l
          WHERE l.loan_id = $1 AND l.reason_code = $2
            AND NOT EXISTS (
                SELECT 1 FROM public.credit_score_log d
                 WHERE d.loan_id = l.loan_id AND d.user_id = l.user_id
                   AND d.reason_code = $3
            )",
    )
    .bind(loan_id)
    .bind(reason::GUARANTOR_CLAIMED)
    .bind(reason::GUARANTOR_CLAIM_SETTLED)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| db_err(e, "claimed guarantors"))?;

    for guarantor_id in claimed {
        let old_score: Option<i16> = sqlx::query_scalar(
            "SELECT score FROM public.credit_scores WHERE user_id = $1 FOR UPDATE",
        )
        .bind(guarantor_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| db_err(e, "guarantor score read"))?;

        let Some(old_score) = old_score else { continue };
        let new_score = (old_score + SCORE_RESTORE_ON_GUARANTOR_SETTLE).min(SCORE_MAX);
        if new_score == old_score {
            continue;
        }

        sqlx::query(
            "UPDATE public.credit_scores SET score = $1, updated_at = $2 WHERE user_id = $3",
        )
        .bind(new_score)
        .bind(now)
        .bind(guarantor_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| db_err(e, "restore guarantor score"))?;

        sqlx::query(
            "INSERT INTO public.credit_score_log
                 (user_id, old_score, new_score, actor_id, reason, reason_code, loan_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(guarantor_id)
        .bind(old_score)
        .bind(new_score)
        .bind(actor_id)
        .bind(format!("claim on loan {loan_id} settled"))
        .bind(reason::GUARANTOR_CLAIM_SETTLED)
        .bind(loan_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| db_err(e, "guarantor restore log"))?;

        tracing::info!(%loan_id, %guarantor_id, old_score, new_score, "guarantor claim penalty partly returned");
    }

    Ok(())
}
