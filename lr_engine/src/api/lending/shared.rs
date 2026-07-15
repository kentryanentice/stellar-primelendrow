//! Plumbing shared by the lending handlers: error mapping, input guards,
//! and the one disbursement routine every product funnels through.

use axum::http::StatusCode;
use chrono::Utc;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::domain;
use super::ledger::{EventDraft, LedgerError, Posting, account_balance, commit_event};
use super::lots;
use crate::api::users::shared::E;

pub fn db_err(e: sqlx::Error, ctx: &'static str) -> E {
    tracing::error!("DB {ctx}: {e}");
    (StatusCode::INTERNAL_SERVER_ERROR, "Unable to process request")
}

pub fn ledger_err(e: LedgerError, ctx: &'static str) -> E {
    match e {
        LedgerError::Unbalanced(net) => {
            tracing::error!("ledger {ctx}: unbalanced by {net}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Unable to process request")
        }
        LedgerError::DuplicateRail => (
            // Idempotent money-in: the rail already credited this reference.
            StatusCode::CONFLICT,
            "This payment was already processed",
        ),
        LedgerError::Db(e) => db_err(e, ctx),
    }
}

/// Whitelist the product string once, at the door (L1).
pub fn validate_product(product: &str) -> Result<&'static str, E> {
    match product {
        "deposit_backed" => Ok("deposit_backed"),
        "xlm_collateral" => Ok("xlm_collateral"),
        "guarantor" => Ok("guarantor"),
        _ => Err((StatusCode::UNPROCESSABLE_ENTITY, "Unknown loan product")),
    }
}

/// Centavo amounts arrive as JSON integers; anything not strictly positive
/// (or absurdly large) stops here.
pub fn validate_centavos(amount: i64) -> Result<i64, E> {
    if amount <= 0 || amount > 1_000_000_000_000 {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Invalid amount"));
    }
    Ok(amount)
}

/// Takes the pool lock, checks real cash, marks funding lots, writes the
/// disbursement into the books, activates the loan, and pins its schedule —
/// all inside the caller's transaction. Every product ends its apply path
/// here; nothing else activates a loan.
pub async fn disburse(
    tx: &mut Transaction<'_, Postgres>,
    loan_id: Uuid,
    borrower_id: Uuid,
    principal: i64,
    rate_bps: i32,
    term_months: i16,
) -> Result<(), E> {
    // The pool's serialization point (D10): one disburse/withdraw at a time
    // decides against the same cash number.
    sqlx::query("SELECT id FROM public.pool_control WHERE id = 1 FOR UPDATE")
        .fetch_one(&mut **tx)
        .await
        .map_err(|e| db_err(e, "pool lock"))?;

    let cash = account_balance(&mut **tx, "cash")
        .await
        .map_err(|e| db_err(e, "cash balance"))?;
    if cash < principal {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "The pool doesn't have enough liquidity right now — try a smaller amount or come back later",
        ));
    }

    // The depositor-visible side of funding: their available lots go 'lent'
    // until principal comes back.
    lots::freeze_funding_lots(tx, principal, loan_id).await?;

    let now = Utc::now().timestamp();
    commit_event(
        tx,
        EventDraft {
            kind: "loan_disbursed",
            user_id: Some(borrower_id),
            loan_id: Some(loan_id),
            deposit_id: None,
            rail_ref: None,
            payload: serde_json::json!({
                "principal": principal, "rate_bps": rate_bps, "term_months": term_months
            }),
            actor_id: Some(borrower_id),
        },
        &[
            Posting { account: "loans_receivable", amount: principal },
            Posting { account: "cash", amount: -principal },
        ],
    )
    .await
    .map_err(|e| ledger_err(e, "loan_disbursed"))?;

    sqlx::query(
        "UPDATE public.loans
            SET status = 'active', principal_outstanding = $1, disbursed_at = $2, updated_at = $2
          WHERE id = $3",
    )
    .bind(principal)
    .bind(now)
    .bind(loan_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| db_err(e, "activate loan"))?;

    for row in domain::build_schedule(principal, rate_bps, term_months, now) {
        sqlx::query(
            "INSERT INTO public.loan_schedule
                (loan_id, installment, due_at, principal_due, interest_due)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(loan_id)
        .bind(row.installment)
        .bind(row.due_at)
        .bind(row.principal_due)
        .bind(row.interest_due)
        .execute(&mut **tx)
        .await
        .map_err(|e| db_err(e, "insert schedule"))?;
    }
    Ok(())
}
