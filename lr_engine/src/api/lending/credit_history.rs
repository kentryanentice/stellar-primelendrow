//! GET /credit/history — a member's own credit score, explained: every change
//! it has been through, the rises already earned and waiting on a term, and
//! what each loan still open could yet do to it.
//!
//! Three lists, because they are three different kinds of fact:
//!
//!   changes  — what happened, read straight off `credit_score_log`, each with
//!              the score it moved from and to.
//!   upcoming — rises a loan repaid in full has already earned, scheduled for
//!              its term end (047). Owed rather than hoped for, so each is
//!              projected on top of the ones due before it.
//!   at_stake — what a loan still running could yet do, one outcome per way it
//!              can end, each projected from today's score alone. A what-if
//!              per outcome, not a forecast: nothing here guesses which way a
//!              loan will go.
//!
//! The deltas are the constants the writers apply (`score`, `admin::default`,
//! `admin::reconcile`), and `outcome` clamps the way those writers clamp, so
//! the page cannot promise a movement the engine would not make.
//!
//! The member's own record only. A borrower's guarantors appear as a count —
//! who else a default could cost — never by name.

use axum::{Extension, Json, http::HeaderMap};
use chrono::Utc;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::admin::{default::SCORE_PENALTY_ON_DEFAULT, reconcile::SCORE_RESTORE_ON_RECONCILE};
use super::score::{
    SCORE_BUMP_ON_CLOSE, SCORE_MAX, SCORE_MIN, SCORE_PENALTY_ON_CLAIM, SCORE_RESTORE_ON_GUARANTOR_SETTLE, reason,
};
use super::shared::db_err;
use crate::api::users::shared::{E, require_verified_user};

/// Newest first. The log is append-only and never pruned, so an unbounded read
/// would only ever grow; fifty explains any score a sprint account can reach.
const HISTORY_LIMIT: i64 = 50;

#[derive(Serialize)]
pub struct ScoreChange {
    /// None on the account's opening row, which has nothing before it.
    pub score_from: Option<i16>,
    pub score_to: i16,
    /// The code from `score::reason` (048).
    pub reason: Option<String>,
    /// The sentence written with the change, sent only where there is no code:
    /// the account's opening row, and rows from before 048.
    pub note: Option<String>,
    /// The loan that caused it, where the row records one (048 onward).
    pub loan_id: Option<Uuid>,
    pub at: i64,
}

#[derive(Serialize)]
pub struct UpcomingRise {
    pub loan_id: Uuid,
    /// When the term-end sweep pays it (`loans.score_rise_at`).
    pub due_at: i64,
    pub paid_off_at: Option<i64>,
    pub term_end: Option<i64>,
    /// Where it takes the score if nothing else lands first, projected on top
    /// of every rise due before it.
    pub score_from: i16,
    pub score_to: i16,
}

/// One way an open loan can still end, and what it would do to the score.
#[derive(Serialize)]
pub struct ScoreOutcome {
    /// The `score::reason` code the change would be logged under.
    pub reason: &'static str,
    /// From today's score. Clamped, so a rise near the top or a claim near the
    /// floor shows what would really move rather than the nominal amount.
    pub delta: i16,
    pub score_to: i16,
}

#[derive(Serialize)]
pub struct ScoreAtStake {
    pub loan_id: Uuid,
    /// `borrower` or `guarantor`: the member's side of the loan.
    pub role: &'static str,
    /// `active`, `defaulted` or `reconciling`.
    pub status: String,
    pub term_end: Option<i64>,
    /// The oldest installment past due and still unpaid. Missing a payment
    /// costs no score by itself — only a declared default does — but it is
    /// the warning that one may come.
    pub overdue_since: Option<i64>,
    /// Accepted pledges behind a borrower's own loan: who else a default could
    /// cost. Zero on a guarantor's row.
    pub guarantors: i64,
    pub outcomes: Vec<ScoreOutcome>,
}

#[derive(Serialize)]
pub struct CreditHistoryResponse {
    pub score: i16,
    pub changes: Vec<ScoreChange>,
    pub upcoming: Vec<UpcomingRise>,
    pub at_stake: Vec<ScoreAtStake>,
}

/// What an outcome would move `score` to, clamped the way the code that
/// applies it clamps: rises stop at 150 (`score`, `admin::reconcile`), a claim
/// cannot take a guarantor below 50 (`score`), and a default floors at 0
/// (`admin::default`) — the one movement that can leave the 50–150 band.
fn outcome(code: &'static str, score: i16) -> ScoreOutcome {
    let to = match code {
        reason::REPAID_TERM_COMPLETE => (score + SCORE_BUMP_ON_CLOSE).min(SCORE_MAX),
        reason::LOAN_DEFAULTED => (score - SCORE_PENALTY_ON_DEFAULT).max(0),
        reason::DEFAULT_SETTLED => (score + SCORE_RESTORE_ON_RECONCILE).min(SCORE_MAX),
        reason::GUARANTOR_CLAIMED => (score - SCORE_PENALTY_ON_CLAIM).max(SCORE_MIN),
        reason::GUARANTOR_CLAIM_SETTLED => (score + SCORE_RESTORE_ON_GUARANTOR_SETTLE).min(SCORE_MAX),
        _ => score,
    };
    ScoreOutcome { reason: code, delta: to - score, score_to: to }
}

/// old_score, new_score, reason_code, reason, loan_id, created_at
type ChangeRow = (Option<i16>, i16, Option<String>, Option<String>, Option<Uuid>, i64);
/// loan id, score_rise_at, closed_at, term end
type DueRow = (Uuid, i64, Option<i64>, Option<i64>);
/// loan id, status, term end, overdue since, accepted guarantors
type BorrowedRow = (Uuid, String, Option<i64>, Option<i64>, i64);
/// loan id, loan status, pledge status, term end, overdue since, claim charged
/// and not yet given back
type PledgedRow = (Uuid, String, String, Option<i64>, Option<i64>, bool);

pub async fn history(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
) -> Result<Json<CreditHistoryResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;
    let now = Utc::now().timestamp();

    // Every account gets a row from the trigger in 015; one that slipped it
    // reads as the opening 50, the same as `credit::status`.
    let score: i16 = sqlx::query_scalar("SELECT score FROM public.credit_scores WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| db_err(e, "credit score"))?
        .unwrap_or(50);

    let rows: Vec<ChangeRow> = sqlx::query_as(
        "SELECT old_score, new_score, reason_code, reason, loan_id, created_at
           FROM public.credit_score_log
          WHERE user_id = $1
          ORDER BY created_at DESC, id DESC
          LIMIT $2",
    )
    .bind(user_id)
    .bind(HISTORY_LIMIT)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "credit score history"))?;
    let changes = rows
        .into_iter()
        .map(|(score_from, score_to, code, sentence, loan_id, at)| ScoreChange {
            score_from,
            score_to,
            note: if code.is_none() { sentence } else { None },
            reason: code,
            loan_id,
            at,
        })
        .collect();

    // Rises earned and not yet paid, in the order the sweep will pay them —
    // which is what lets each be projected from the one before.
    let due: Vec<DueRow> = sqlx::query_as(
        "SELECT l.id, l.score_rise_at, l.closed_at,
                (SELECT MAX(s.due_at) FROM public.loan_schedule s WHERE s.loan_id = l.id)
           FROM public.loans l
          WHERE l.borrower_id = $1 AND l.status = 'closed'
            AND l.score_rise_at IS NOT NULL AND l.score_awarded_at IS NULL
          ORDER BY l.score_rise_at, l.id",
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "upcoming score rises"))?;
    let mut upcoming = Vec::with_capacity(due.len());
    let mut running = score;
    for (loan_id, due_at, paid_off_at, term_end) in due {
        let rise = outcome(reason::REPAID_TERM_COMPLETE, running);
        upcoming.push(UpcomingRise {
            loan_id, due_at, paid_off_at, term_end, score_from: running, score_to: rise.score_to,
        });
        running = rise.score_to;
    }

    // The member's own loans that can still move their score: a running loan
    // either way, a default only back up.
    let borrowed: Vec<BorrowedRow> = sqlx::query_as(
        "SELECT l.id, l.status,
                (SELECT MAX(s.due_at) FROM public.loan_schedule s WHERE s.loan_id = l.id),
                (SELECT MIN(s.due_at) FROM public.loan_schedule s
                  WHERE s.loan_id = l.id AND s.status = 'scheduled' AND s.due_at < $2),
                (SELECT COUNT(*) FROM public.loan_guarantors g
                  WHERE g.loan_id = l.id AND g.status = 'accepted')
           FROM public.loans l
          WHERE l.borrower_id = $1 AND l.status IN ('active', 'defaulted', 'reconciling')
          ORDER BY l.created_at DESC, l.id",
    )
    .bind(user_id)
    .bind(now)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "loans at stake"))?;
    let mut at_stake: Vec<ScoreAtStake> = borrowed
        .into_iter()
        .map(|(loan_id, status, term_end, overdue_since, guarantors)| {
            let outcomes = if status == "active" {
                vec![outcome(reason::REPAID_TERM_COMPLETE, score), outcome(reason::LOAN_DEFAULTED, score)]
            } else {
                vec![outcome(reason::DEFAULT_SETTLED, score)]
            };
            ScoreAtStake { loan_id, role: "borrower", status, term_end, overdue_since, guarantors, outcomes }
        })
        .collect();

    // Pledges that can still move the member's score as a guarantor: one still
    // locked can be claimed, and a claim already charged comes partly back if
    // the default is settled — the same test `restore_guarantor_claims` makes
    // before it pays.
    let pledged: Vec<PledgedRow> = sqlx::query_as(
        "SELECT l.id, l.status, g.status,
                (SELECT MAX(s.due_at) FROM public.loan_schedule s WHERE s.loan_id = l.id),
                (SELECT MIN(s.due_at) FROM public.loan_schedule s
                  WHERE s.loan_id = l.id AND s.status = 'scheduled' AND s.due_at < $2),
                EXISTS(SELECT 1 FROM public.credit_score_log c
                        WHERE c.loan_id = l.id AND c.user_id = $1 AND c.reason_code = $3)
                AND NOT EXISTS(SELECT 1 FROM public.credit_score_log c
                        WHERE c.loan_id = l.id AND c.user_id = $1 AND c.reason_code = $4)
           FROM public.loan_guarantors g
           JOIN public.loans l ON l.id = g.loan_id
          WHERE g.guarantor_id = $1 AND l.status IN ('active', 'defaulted', 'reconciling')
          ORDER BY l.created_at DESC, l.id",
    )
    .bind(user_id)
    .bind(now)
    .bind(reason::GUARANTOR_CLAIMED)
    .bind(reason::GUARANTOR_CLAIM_SETTLED)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "pledges at stake"))?;
    for (loan_id, status, pledge, term_end, overdue_since, claimed) in pledged {
        let mut outcomes = Vec::new();
        if pledge == "accepted" {
            outcomes.push(outcome(reason::GUARANTOR_CLAIMED, score));
        }
        if claimed {
            outcomes.push(outcome(reason::GUARANTOR_CLAIM_SETTLED, score));
        }
        if !outcomes.is_empty() {
            at_stake.push(ScoreAtStake {
                loan_id, role: "guarantor", status, term_end, overdue_since, guarantors: 0, outcomes,
            });
        }
    }

    Ok(Json(CreditHistoryResponse { score, changes, upcoming, at_stake }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each outcome lands where its writer would put it, including at the
    /// edges where the writer clamps.
    #[test]
    fn outcomes_move_the_score_the_way_their_writers_do() {
        let to = |code, score| outcome(code, score).score_to;
        assert_eq!(to(reason::REPAID_TERM_COMPLETE, 60), 65);
        assert_eq!(to(reason::REPAID_TERM_COMPLETE, 148), SCORE_MAX);
        // A default is the one movement that can leave the band.
        assert_eq!(to(reason::LOAN_DEFAULTED, 60), 35);
        assert_eq!(to(reason::LOAN_DEFAULTED, 10), 0);
        assert_eq!(to(reason::DEFAULT_SETTLED, 35), 55);
        assert_eq!(to(reason::GUARANTOR_CLAIMED, 80), 70);
        assert_eq!(to(reason::GUARANTOR_CLAIMED, 55), SCORE_MIN);
        assert_eq!(to(reason::GUARANTOR_CLAIM_SETTLED, 70), 75);
    }

    /// The delta is what would really move, not the nominal amount.
    #[test]
    fn a_clamped_outcome_reports_the_clamped_delta() {
        assert_eq!(outcome(reason::REPAID_TERM_COMPLETE, 148).delta, 2);
        assert_eq!(outcome(reason::GUARANTOR_CLAIMED, 55).delta, -5);
        assert_eq!(outcome(reason::LOAN_DEFAULTED, 60).delta, -25);
    }
}
