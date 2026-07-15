//! POST /loans/repay — PHP money-in against an active loan.
//!
//! PayPal is captured server-side first (never inside the transaction); the
//! captured centavos are then allocated oldest installment first — each
//! installment's interest then its principal — so a payment clears whole
//! installments in order (Lesson 8), the interest is split at the
//! single rounding site, 'lent' funding lots unlock as principal returns,
//! and a fully paid loan releases its collateral/pledges and bumps the
//! borrower's score. A duplicate capture bounces off the ledger's rail_ref.

use axum::{Extension, Json, http::{HeaderMap, StatusCode}};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::domain;
use super::ledger::{EventDraft, Posting, commit_event};
use super::lots;
use super::policy;
use super::shared::{db_err, ledger_err};
use crate::api::users::shared::{E, require_verified_user};
use crate::infra::paypal;

/// Score movement on a fully repaid loan. Kept here (not policy JSON) until
/// scoring gets its own policy slice — it's one number, and the log records
/// every application of it.
const SCORE_BUMP_ON_CLOSE: i16 = 5;

#[derive(Deserialize)]
pub struct RepayInput {
    loan_id: Uuid,
    order_id: String,
}

#[derive(Serialize)]
pub struct RepayResponse {
    pub amount_received: i64,
    pub interest_paid: i64,
    pub principal_paid: i64,
    /// Anything beyond what the loan owed becomes a fresh deposit lot.
    pub excess_to_deposit: i64,
    pub principal_outstanding: i64,
    pub loan_status: &'static str,
    pub message: &'static str,
}

struct ScheduleRow {
    id: i64,
    interest_due: i64,
    interest_paid: i64,
    principal_due: i64,
    principal_paid: i64,
}

pub async fn repay(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(p): Json<RepayInput>,
) -> Result<Json<RepayResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    let rules = policy::active(&pool).await?;

    // Capture before the transaction: no locks held across PayPal.
    let captured = paypal::capture_order(p.order_id.trim())
        .await
        .map_err(|m| (StatusCode::UNPROCESSABLE_ENTITY, m))?;
    let received = captured.centavos;

    let mut tx = pool.begin().await.map_err(|e| db_err(e, "begin repay"))?;

    let loan: Option<(Uuid, String, i64, String)> = sqlx::query_as(
        "SELECT borrower_id, product, principal_outstanding, status
           FROM public.loans WHERE id = $1 FOR UPDATE",
    )
    .bind(p.loan_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| db_err(e, "lock loan"))?;
    let (borrower_id, product, outstanding_before, status) =
        loan.ok_or((StatusCode::NOT_FOUND, "Loan not found"))?;
    if borrower_id != user_id {
        return Err((StatusCode::NOT_FOUND, "Loan not found"));
    }
    if status != "active" {
        return Err((StatusCode::CONFLICT, "This loan is not active"));
    }

    let rows: Vec<(i64, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT id, interest_due, interest_paid, principal_due, principal_paid
           FROM public.loan_schedule
          WHERE loan_id = $1
          ORDER BY installment
          FOR UPDATE",
    )
    .bind(p.loan_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| db_err(e, "lock schedule"))?;
    let mut schedule: Vec<ScheduleRow> = rows
        .into_iter()
        .map(|(id, interest_due, interest_paid, principal_due, principal_paid)| ScheduleRow {
            id, interest_due, interest_paid, principal_due, principal_paid,
        })
        .collect();

    // Allocation, oldest installment first: within each installment settle its
    // interest THEN its principal before moving to the next. A payment clears
    // whole installments in order — overdue interest + principal, then the
    // current one, then future ones — instead of vacuuming every month's
    // interest across the whole loan first (which pre-paid interest that wasn't
    // due and left each "paid" installment still owing principal). Interest is
    // still paid before principal within an installment (Lesson 8). Anything
    // past the final installment is excess and becomes a deposit lot.
    let mut remaining = received;
    let mut interest_total: i64 = 0;
    let mut principal_total: i64 = 0;
    for row in &mut schedule {
        if remaining == 0 {
            break;
        }
        let interest_owed = row.interest_due - row.interest_paid;
        if interest_owed > 0 {
            let pay = interest_owed.min(remaining);
            row.interest_paid += pay;
            interest_total += pay;
            remaining -= pay;
        }
        if remaining == 0 {
            break;
        }
        let principal_owed = row.principal_due - row.principal_paid;
        if principal_owed > 0 {
            let pay = principal_owed.min(remaining);
            row.principal_paid += pay;
            principal_total += pay;
            remaining -= pay;
        }
    }
    let excess = remaining;

    let now = Utc::now().timestamp();
    for row in &schedule {
        let settled = row.interest_paid >= row.interest_due && row.principal_paid >= row.principal_due;
        sqlx::query(
            "UPDATE public.loan_schedule
                SET interest_paid = $1, principal_paid = $2,
                    status = CASE WHEN $3 THEN 'paid' ELSE status END
              WHERE id = $4",
        )
        .bind(row.interest_paid)
        .bind(row.principal_paid)
        .bind(settled)
        .bind(row.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| db_err(e, "update schedule"))?;
    }

    // The books: one event, postings that tie to the received centavo.
    let (platform_cut, reserve_cut) =
        domain::split_interest(interest_total, &rules.params.interest_split);
    let mut postings = vec![Posting { account: "cash", amount: received }];
    if principal_total > 0 {
        postings.push(Posting { account: "loans_receivable", amount: -principal_total });
    }
    if platform_cut > 0 {
        postings.push(Posting { account: "platform_earnings", amount: -platform_cut });
    }
    if reserve_cut > 0 {
        postings.push(Posting { account: "reserve_fund", amount: -reserve_cut });
    }
    if excess > 0 {
        postings.push(Posting { account: "member_deposits", amount: -excess });
    }

    commit_event(
        &mut tx,
        EventDraft {
            kind: "repayment_received",
            user_id: Some(user_id),
            loan_id: Some(p.loan_id),
            deposit_id: None,
            rail_ref: Some(captured.capture_id.clone()),
            payload: serde_json::json!({
                "received": received, "interest": interest_total,
                "principal": principal_total, "excess": excess
            }),
            actor_id: Some(user_id),
        },
        &postings,
    )
    .await
    .map_err(|e| ledger_err(e, "repayment"))?;

    // The borrower-facing payment record (migration 023): the ledger event
    // above is the books, this is the "here is every payment you made" row the
    // Pay page reads. The ledger's unique rail_ref already refused any double
    // credit, so by here this capture is known-unique.
    sqlx::query(
        "INSERT INTO public.loan_payments
            (loan_id, user_id, amount_received, interest_paid, principal_paid, excess, rail_ref, paid_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(p.loan_id)
    .bind(user_id)
    .bind(received)
    .bind(interest_total)
    .bind(principal_total)
    .bind(excess)
    .bind(&captured.capture_id)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|e| db_err(e, "record payment"))?;

    // Overpay becomes a withdrawable deposit lot instead of vanishing.
    if excess > 0 {
        sqlx::query("INSERT INTO public.deposits (user_id, amount, badge) VALUES ($1, $2, 'available')")
            .bind(user_id)
            .bind(excess)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_err(e, "excess lot"))?;
    }

    let outstanding_after = outstanding_before - principal_total;
    sqlx::query("UPDATE public.loans SET principal_outstanding = $1, updated_at = $2 WHERE id = $3")
        .bind(outstanding_after)
        .bind(now)
        .bind(p.loan_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| db_err(e, "update outstanding"))?;

    // Principal came home -> that much funded deposit is withdrawable again.
    if principal_total > 0 {
        lots::release_funding_lots(&mut tx, p.loan_id, principal_total).await?;
    }

    let fully_paid = outstanding_after == 0
        && schedule.iter().all(|r| r.interest_paid >= r.interest_due);

    let loan_status = if fully_paid {
        sqlx::query("UPDATE public.loans SET status = 'closed', closed_at = $1, updated_at = $1 WHERE id = $2")
            .bind(now)
            .bind(p.loan_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_err(e, "close loan"))?;

        // Collateral goes home, whatever shape it took.
        lots::release_loan_lots(&mut tx, p.loan_id, &["collateral", "pledged"]).await?;
        sqlx::query(
            "UPDATE public.loan_guarantors SET status = 'released', updated_at = $1
              WHERE loan_id = $2 AND status = 'accepted'",
        )
        .bind(now)
        .bind(p.loan_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| db_err(e, "release guarantors"))?;

        if product == "xlm_collateral" {
            // The DB claim flips now; the on-chain release is queued for the
            // admin key (the contract refuses anyone else) — backend gated
            // end to end.
            let collateral_id: Option<Uuid> = sqlx::query_scalar(
                "UPDATE public.xlm_collateral SET status = 'released', updated_at = $1
                  WHERE loan_id = $2 AND status = 'locked'
                  RETURNING id",
            )
            .bind(now)
            .bind(p.loan_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| db_err(e, "release collateral"))?;
            if let Some(cid) = collateral_id {
                sqlx::query(
                    "INSERT INTO public.collateral_actions (collateral_id, action) VALUES ($1, 'release')",
                )
                .bind(cid)
                .execute(&mut *tx)
                .await
                .map_err(|e| db_err(e, "queue release"))?;
            }
        }

        // Track record moves on real behavior (D5): repaid in full = +score,
        // logged like every other score change.
        let old_score: Option<i16> = sqlx::query_scalar(
            "SELECT score FROM public.credit_scores WHERE user_id = $1 FOR UPDATE",
        )
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| db_err(e, "score read"))?;
        if let Some(old_score) = old_score {
            let new_score = (old_score + SCORE_BUMP_ON_CLOSE).min(150);
            if new_score != old_score {
                sqlx::query("UPDATE public.credit_scores SET score = $1, updated_at = $2 WHERE user_id = $3")
                    .bind(new_score)
                    .bind(now)
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| db_err(e, "bump score"))?;
                sqlx::query(
                    "INSERT INTO public.credit_score_log (user_id, old_score, new_score, actor_id, reason)
                     VALUES ($1, $2, $3, NULL, $4)",
                )
                .bind(user_id)
                .bind(old_score)
                .bind(new_score)
                .bind(format!("loan {} repaid in full", p.loan_id))
                .execute(&mut *tx)
                .await
                .map_err(|e| db_err(e, "score log"))?;
            }
        }

        commit_event(
            &mut tx,
            EventDraft {
                kind: "loan_closed",
                user_id: Some(user_id),
                loan_id: Some(p.loan_id),
                deposit_id: None,
                rail_ref: None,
                payload: serde_json::json!({ "product": product }),
                actor_id: Some(user_id),
            },
            &[],
        )
        .await
        .map_err(|e| ledger_err(e, "loan_closed"))?;
        "closed"
    } else {
        "active"
    };

    tx.commit().await.map_err(|e| db_err(e, "commit repay"))?;
    tracing::info!(%user_id, loan_id = %p.loan_id, received, interest_total, principal_total, "repayment received");

    Ok(Json(RepayResponse {
        amount_received: received,
        interest_paid: interest_total,
        principal_paid: principal_total,
        excess_to_deposit: excess,
        principal_outstanding: outstanding_after,
        loan_status,
        message: if loan_status == "closed" {
            "Loan fully repaid — collateral released and your credit score just went up"
        } else {
            "Payment received and applied"
        },
    }))
}
