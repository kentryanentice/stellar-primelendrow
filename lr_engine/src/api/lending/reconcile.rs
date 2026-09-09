//! Settling a defaulted loan — POST /lending/admin/loans/reopen
//!                             POST /lending/admin/loans/reconcile
//!
//! A default is the end of the road for a borrower who walked away. This is the
//! road back for one who wants to make it good, and it is deliberately three
//! moves rather than one button:
//!
//!   1. **the admin reopens it** (`reopen`) — a judgement that this borrower
//!      should be allowed to settle. The loan becomes `reconciling`, its
//!      defaulted installments become due again, and the arrears appear on the
//!      borrower's own Pay page.
//!   2. **the borrower pays** (`repay`, unchanged endpoint) — through the
//!      ordinary PayPal/Stripe rail, verified by the provider like every other
//!      peso the pool takes in. `settle` below decides where that money goes.
//!   3. **the admin marks it paid** (`mark_paid`) — once the arrears are
//!      actually clear. The loan becomes `reconciled`, the credit penalty is
//!      returned, and the on-chain outcome is queued for the vault admin.
//!
//! Splitting it this way keeps the administrator's power where it belongs. They
//! decide *whether* a borrower may settle and *whether* the loan is square.
//! They never assert that money arrived — only a captured payment does that, so
//! there is no path where an admin's click credits pesos nobody paid.
//!
//! **Where settlement money goes, and why it isn't a repayment.** By the time a
//! loan defaults the debt is already written off: `recovery::advance` drove
//! `loans_receivable` to zero, taking the borrower's deposits, seizing coins,
//! charging guarantors and booking the rest as a `reserve_fund` loss. A peso
//! arriving now cannot pay down a receivable that no longer exists — posting it
//! that way would drive the pool's assets negative. It undoes the loss instead,
//! and the order is a promise about who matters most:
//!
//!   1. **guarantors**, refunded as fresh deposit lots. They were charged for
//!      somebody else's debt and are the only genuinely innocent party here.
//!   2. **the pool's `reserve_fund`**, restored.
//!   3. **the borrower**, anything left over, as a deposit lot.
//!
//! Note this is *not* the strict reverse of the waterfall. The waterfall
//! charges the pool last, so a strict unwind would refund the pool first — the
//! house making itself whole ahead of the third parties it drew on. When a
//! partial settlement can't cover everyone, the house waits.
//!
//! Two things are never given back. The **borrower's own seized deposits**:
//! that money legitimately paid their own debt, and returning it while they
//! also pay the settlement would pay them twice. And **seized XLM**: those
//! coins moved on-chain, and only the chain can move them back — `mark_paid`
//! reports the position rather than pretending to reverse it.

use axum::{Extension, Json, http::{HeaderMap, StatusCode}};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::ledger::{EventDraft, commit_event};
use super::shared::{db_err, ledger_err};
use crate::api::users::shared::{E, require_admin};

/// Returned when a defaulted loan is made good. The mirror of
/// `default_loan::SCORE_PENALTY_ON_DEFAULT`, and the same number on purpose: a
/// settled default should leave the borrower where they stood, not ahead of it.
/// It is not paired with `repay`'s +5 for a clean close — settling late is not
/// the same as paying on time, and only the penalty is undone.
const SCORE_RESTORE_ON_RECONCILE: i16 = 25;

// ===========================================================================
// What is still owed
// ===========================================================================

/// Everything the borrower has not paid on this loan, principal and interest,
/// across every installment that isn't settled.
///
/// This is the number the whole feature turns on: it is what the admin quotes,
/// what the borrower sees on the Pay page, and what `mark_paid` requires to be
/// zero. It is read from `loan_schedule` rather than `principal_outstanding`,
/// because recovery drove that column to zero — the schedule is the only place
/// that still remembers what was actually missed.
pub(super) async fn arrears(
    tx: &mut Transaction<'_, Postgres>,
    loan_id: Uuid,
) -> Result<i64, E> {
    let owed: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(
                    GREATEST(principal_due - principal_paid, 0)
                  + GREATEST(interest_due - interest_paid, 0)
                ), 0)::BIGINT
           FROM public.loan_schedule
          WHERE loan_id = $1 AND status <> 'paid'",
    )
    .bind(loan_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| db_err(e, "loan arrears"))?;
    Ok(owed)
}

// ===========================================================================
// 1. Reopening
// ===========================================================================

#[derive(Deserialize)]
pub struct ReopenInput {
    loan_id: Uuid,
    /// Why this borrower is being allowed to settle. Required — reversing a
    /// credit consequence is exactly the decision that should never be
    /// anonymous in the audit trail.
    #[serde(default)]
    reason: String,
}

#[derive(Serialize)]
pub struct ReopenResponse {
    pub loan_id: Uuid,
    /// What the borrower now has to pay, in centavos.
    pub arrears: i64,
    pub message: &'static str,
}

pub async fn reopen(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(p): Json<ReopenInput>,
) -> Result<Json<ReopenResponse>, E> {
    let admin_id = require_admin(&pool, &headers).await?;

    let reason = p.reason.trim().chars().take(200).collect::<String>();
    if reason.is_empty() {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "Give a reason — reopening a defaulted loan is recorded against your account",
        ));
    }

    let mut tx = pool.begin().await.map_err(|e| db_err(e, "begin reopen"))?;

    // The loan row serializes two admins clicking at once; the loser sees the
    // status has moved and is refused.
    let loan: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT borrower_id, status FROM public.loans WHERE id = $1 FOR UPDATE",
    )
    .bind(p.loan_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| db_err(e, "lock loan"))?;
    let (borrower_id, status) = loan.ok_or((StatusCode::NOT_FOUND, "No such loan"))?;
    if status != "defaulted" {
        return Err((
            StatusCode::CONFLICT,
            "Only a defaulted loan can be reopened for settlement",
        ));
    }

    // A borrower may settle one old debt at a time, and must not be doing it
    // while a fresh loan is running — that would be two open obligations, which
    // is the thing `apply`'s one-open-loan rule exists to prevent.
    let has_open: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM public.loans
          WHERE borrower_id = $1 AND status IN ('pending', 'active', 'reconciling'))",
    )
    .bind(borrower_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| db_err(e, "open loan check"))?;
    if has_open {
        return Err((
            StatusCode::CONFLICT,
            "This borrower already has an open loan or another settlement in progress",
        ));
    }

    let now = Utc::now().timestamp();

    // Payable again. `closed_at` is cleared because recovery is no longer
    // finished — leaving it set would say this loan was done while money is
    // still expected against it.
    sqlx::query(
        "UPDATE public.loans
            SET status = 'reconciling', reconcile_opened_at = $1, closed_at = NULL, updated_at = $1
          WHERE id = $2",
    )
    .bind(now)
    .bind(p.loan_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| db_err(e, "reopen loan"))?;

    // The installments the default called in are scheduled again — 'scheduled'
    // is the status these rows carried before the default stamped them, and the
    // only other value the column takes for an unpaid row ('late' is set by
    // age, not here). A row already 'paid' is untouched: the borrower did pay
    // it, and neither defaulting nor reopening rewrites that.
    sqlx::query(
        "UPDATE public.loan_schedule SET status = 'scheduled'
          WHERE loan_id = $1 AND status = 'defaulted'",
    )
    .bind(p.loan_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| db_err(e, "reopen schedule"))?;

    let owed = arrears(&mut tx, p.loan_id).await?;

    // No postings: reopening moves no money. Everything that moves is posted by
    // `settle`, against a payment the provider actually confirmed.
    commit_event(
        &mut tx,
        EventDraft {
            kind: "loan_reopened_for_settlement",
            user_id: Some(borrower_id),
            loan_id: Some(p.loan_id),
            deposit_id: None,
            rail_ref: None,
            payload: serde_json::json!({ "arrears": owed, "reason": reason }),
            actor_id: Some(admin_id),
        },
        &[],
    )
    .await
    .map_err(|e| ledger_err(e, "loan_reopened_for_settlement"))?;

    tx.commit().await.map_err(|e| db_err(e, "commit reopen"))?;
    tracing::info!(%admin_id, loan = %p.loan_id, arrears = owed, "defaulted loan reopened for settlement");

    Ok(Json(ReopenResponse {
        loan_id: p.loan_id,
        arrears: owed,
        message: "Reopened — the borrower can now pay the arrears from their Pay page",
    }))
}

// ===========================================================================
// 2. Where a settlement payment goes
// ===========================================================================

/// How a settlement payment was distributed. Returned to `repay` so it can
/// build the ledger postings and tell the borrower what happened.
pub(super) struct Settlement {
    /// Refunded to guarantors who were charged, total.
    pub to_guarantors: i64,
    /// Returned to the pool's reserve.
    pub to_reserve: i64,
    /// Left over, and handed back to the borrower as a deposit lot.
    pub to_borrower: i64,
}

/// Distributes `received` centavos across the parties the default charged.
///
/// Called by `repay` when the loan is `reconciling`, inside its transaction and
/// under its loan-row lock. Creates the guarantors' refund lots as it goes and
/// records how much of each recovery step has been given back, so a settlement
/// paid in several instalments never refunds the same step twice.
///
/// Returns the split; the caller posts it. Keeping the postings in one place
/// (`repay`) rather than committing an event here is what stops a settlement
/// from being recorded as two half-events if either part fails.
pub(super) async fn settle(
    tx: &mut Transaction<'_, Postgres>,
    loan_id: Uuid,
    borrower_id: Uuid,
    received: i64,
) -> Result<Settlement, E> {
    let mut remaining = received;
    let mut to_guarantors = 0i64;

    // ---- 1. the guarantors, first and in the order they were charged -------
    // `FOR UPDATE` because two settlements arriving at once must not both read
    // the same unrefunded balance and each refund it.
    let charged: Vec<(i64, Option<Uuid>, i64, i64)> = sqlx::query_as(
        "SELECT id, user_id, amount, refunded
           FROM public.loan_recoveries
          WHERE loan_id = $1 AND source = 'guarantor_deposit' AND refunded < amount
          ORDER BY id
          FOR UPDATE",
    )
    .bind(loan_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| db_err(e, "charged guarantors"))?;

    for (recovery_id, user_id, amount, refunded) in charged {
        if remaining == 0 {
            break;
        }
        let Some(user_id) = user_id else { continue };
        let owed_back = amount - refunded;
        let pay = owed_back.min(remaining);
        if pay <= 0 {
            continue;
        }

        // A real lot, not just a liability: a guarantor whose deposit came back
        // as a number with no lot behind it would see nothing on their balance
        // and have nothing to withdraw.
        sqlx::query(
            "INSERT INTO public.deposits (user_id, amount, badge) VALUES ($1, $2, 'available')",
        )
        .bind(user_id)
        .bind(pay)
        .execute(&mut **tx)
        .await
        .map_err(|e| db_err(e, "guarantor refund lot"))?;

        sqlx::query("UPDATE public.loan_recoveries SET refunded = refunded + $1 WHERE id = $2")
            .bind(pay)
            .bind(recovery_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| db_err(e, "record guarantor refund"))?;

        // Their pledge is no longer seized — it was given back.
        sqlx::query(
            "UPDATE public.loan_guarantors SET status = 'released', updated_at = $1
              WHERE loan_id = $2 AND user_id = $3 AND status = 'seized'",
        )
        .bind(Utc::now().timestamp())
        .bind(loan_id)
        .bind(user_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| db_err(e, "release guarantor"))?;

        to_guarantors += pay;
        remaining -= pay;
    }

    // ---- 2. the pool's reserve --------------------------------------------
    let mut to_reserve = 0i64;
    if remaining > 0 {
        let absorbed: Vec<(i64, i64, i64)> = sqlx::query_as(
            "SELECT id, amount, refunded
               FROM public.loan_recoveries
              WHERE loan_id = $1 AND source = 'reserve_fund' AND refunded < amount
              ORDER BY id
              FOR UPDATE",
        )
        .bind(loan_id)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| db_err(e, "reserve absorbed"))?;

        for (recovery_id, amount, refunded) in absorbed {
            if remaining == 0 {
                break;
            }
            let pay = (amount - refunded).min(remaining);
            if pay <= 0 {
                continue;
            }
            sqlx::query("UPDATE public.loan_recoveries SET refunded = refunded + $1 WHERE id = $2")
                .bind(pay)
                .bind(recovery_id)
                .execute(&mut **tx)
                .await
                .map_err(|e| db_err(e, "record reserve refund"))?;
            to_reserve += pay;
            remaining -= pay;
        }
    }

    // ---- 3. whatever is left, back to the borrower ------------------------
    // They have overpaid what the default actually cost anyone. Keeping it
    // would be the pool profiting from a rescued default.
    if remaining > 0 {
        sqlx::query(
            "INSERT INTO public.deposits (user_id, amount, badge) VALUES ($1, $2, 'available')",
        )
        .bind(borrower_id)
        .bind(remaining)
        .execute(&mut **tx)
        .await
        .map_err(|e| db_err(e, "settlement excess lot"))?;
    }

    Ok(Settlement {
        to_guarantors,
        to_reserve,
        to_borrower: remaining,
    })
}

// ===========================================================================
// 3. Marking it paid
// ===========================================================================

#[derive(Deserialize)]
pub struct ReconcileInput {
    loan_id: Uuid,
    #[serde(default)]
    reason: String,
}

#[derive(Serialize)]
pub struct ReconcileResponse {
    pub loan_id: Uuid,
    /// The borrower's score after the penalty was returned.
    pub score: Option<i16>,
    /// True when `mark_repaid` + `release` were queued for the vault admin to
    /// sign — the on-chain half of "mark them paid".
    pub queued_on_chain: bool,
    /// Set when the borrower's coins were already seized on-chain and cannot be
    /// given back from here.
    pub note: Option<&'static str>,
    pub message: &'static str,
}

/// Accepts a reopened loan as settled.
///
/// Refuses while anything is still owed. That refusal is the point of having a
/// separate endpoint at all: an admin decides *whether* the borrower may settle
/// and *whether* to accept it, but the arrears reaching zero is a fact about
/// captured payments, not an administrative opinion.
pub async fn mark_paid(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(p): Json<ReconcileInput>,
) -> Result<Json<ReconcileResponse>, E> {
    let admin_id = require_admin(&pool, &headers).await?;
    let reason = p.reason.trim().chars().take(200).collect::<String>();

    let mut tx = pool.begin().await.map_err(|e| db_err(e, "begin reconcile"))?;

    let loan: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT borrower_id, status FROM public.loans WHERE id = $1 FOR UPDATE",
    )
    .bind(p.loan_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| db_err(e, "lock loan"))?;
    let (borrower_id, status) = loan.ok_or((StatusCode::NOT_FOUND, "No such loan"))?;
    if status != "reconciling" {
        return Err((
            StatusCode::CONFLICT,
            "Reopen the loan for settlement before marking it paid",
        ));
    }

    let owed = arrears(&mut tx, p.loan_id).await?;
    if owed > 0 {
        return Err((
            StatusCode::CONFLICT,
            "There are still arrears on this loan — it can't be marked paid until they're settled",
        ));
    }

    let now = Utc::now().timestamp();
    sqlx::query(
        "UPDATE public.loans
            SET status = 'reconciled', reconciled_at = $1, closed_at = $1, updated_at = $1
          WHERE id = $2",
    )
    .bind(now)
    .bind(p.loan_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| db_err(e, "reconcile loan"))?;

    // Any deposit still frozen against this loan comes home. `lent` is in the
    // list because a reopened loan may still have savers' lots funding it —
    // recovery released those when it settled, but a loan reopened before that
    // point can reach here with them outstanding.
    super::lots::release_loan_lots(&mut tx, p.loan_id, &["collateral", "pledged", "lent"]).await?;

    // The credit consequence, returned. Exactly the penalty a default cost, no
    // more: settling late is not the same as paying on time, so this restores
    // standing rather than rewarding it.
    let old_score: Option<i16> = sqlx::query_scalar(
        "SELECT score FROM public.credit_scores WHERE user_id = $1 FOR UPDATE",
    )
    .bind(borrower_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| db_err(e, "score read"))?;

    let mut score_after = old_score;
    if let Some(old) = old_score {
        let new_score = (old + SCORE_RESTORE_ON_RECONCILE).min(150);
        score_after = Some(new_score);
        if new_score != old {
            sqlx::query("UPDATE public.credit_scores SET score = $1, updated_at = $2 WHERE user_id = $3")
                .bind(new_score)
                .bind(now)
                .bind(borrower_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| db_err(e, "restore score"))?;
            sqlx::query(
                "INSERT INTO public.credit_score_log (user_id, old_score, new_score, actor_id, reason)
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(borrower_id)
            .bind(old)
            .bind(new_score)
            .bind(admin_id)
            .bind(format!("loan {} settled after default", p.loan_id))
            .execute(&mut *tx)
            .await
            .map_err(|e| db_err(e, "score log"))?;
        }
    }

    // The on-chain half. A position still `locked` means the seizure was queued
    // but never signed, so the coins are still in the vault and can go home —
    // `mark_repaid` then `release`, in that order, because the contract refuses
    // to release a position with no repayment recorded against it.
    //
    // A position already `seized` is a different story and is NOT faked: those
    // coins left the vault on-chain, and no row written here would bring them
    // back. The response says so rather than reporting a clean settlement.
    let position: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT id, status FROM public.xlm_collateral WHERE loan_id = $1",
    )
    .bind(p.loan_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| db_err(e, "collateral position"))?;

    let mut queued_on_chain = false;
    let mut note = None;
    match position.as_ref().map(|(id, status)| (*id, status.as_str())) {
        Some((collateral_id, "locked")) => {
            sqlx::query(
                "INSERT INTO public.collateral_actions (collateral_id, action, actor_id)
                 VALUES ($1, 'mark_repaid', $2), ($1, 'release', $2)",
            )
            .bind(collateral_id)
            .bind(admin_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_err(e, "queue release"))?;
            queued_on_chain = true;
        }
        Some((_, "seized")) => {
            note = Some(
                "The borrower's XLM was already seized on-chain — settling here can't return it. Send the coins back manually if that was agreed.",
            );
        }
        _ => {}
    }

    commit_event(
        &mut tx,
        EventDraft {
            kind: "loan_reconciled",
            user_id: Some(borrower_id),
            loan_id: Some(p.loan_id),
            deposit_id: None,
            rail_ref: None,
            payload: serde_json::json!({
                "reason": if reason.is_empty() { serde_json::Value::Null } else { reason.clone().into() },
                "score_after": score_after,
                "queued_on_chain": queued_on_chain,
            }),
            actor_id: Some(admin_id),
        },
        // No postings. Every peso this settlement moved was posted by `settle`
        // against the payment that carried it; accepting the outcome moves
        // nothing, exactly as declaring the default moved nothing.
        &[],
    )
    .await
    .map_err(|e| ledger_err(e, "loan_reconciled"))?;

    tx.commit().await.map_err(|e| db_err(e, "commit reconcile"))?;
    tracing::info!(%admin_id, loan = %p.loan_id, ?score_after, queued_on_chain, "defaulted loan reconciled");

    Ok(Json(ReconcileResponse {
        loan_id: p.loan_id,
        score: score_after,
        queued_on_chain,
        note,
        message: if queued_on_chain {
            "Settled. Sign the queued vault movements to return the borrower's collateral — they can apply again now"
        } else {
            "Settled — the borrower's standing is restored and they can apply again"
        },
    }))
}
