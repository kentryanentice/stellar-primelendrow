//! Plumbing shared by the lending handlers: error mapping, input guards,
//! and the one disbursement routine every product funnels through.

use axum::http::StatusCode;
use chrono::Utc;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::domain;
use super::ledger::{
    EventDraft, LedgerError, Posting, commit_event, free_cash, retained_funds, unwithdrawn_proceeds,
};
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

/// Does this member have a default still open — declared and not yet settled?
///
/// The SOW's "verified and non-defaulted guarantors only" (§4.1), checked both
/// when a borrower names a guarantor and when the guarantor accepts, since a
/// default can land in between. A settled default (`reconciled`) does not
/// count: settling is how a member gets back in, the same line the
/// one-open-loan rule in `apply` draws for borrowing.
pub async fn has_unsettled_default(tx: &mut Transaction<'_, Postgres>, user_id: Uuid) -> Result<bool, E> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM public.loans
          WHERE borrower_id = $1 AND status IN ('defaulted', 'reconciling'))",
    )
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| db_err(e, "unsettled default check"))
}

/// Is every leg backing this loan actually in place?
///
/// Two products fund asynchronously, and since the 50% rule a guarantor loan
/// can be waiting on BOTH at once: the borrower's own XLM has to lock on
/// chain, and the guarantors have to accept enough to cover what the borrower
/// did not carry. Whichever of the two lands last is the one that funds the
/// loan, so both call this rather than each assuming it is the only gate.
///
/// Read inside the caller's transaction, with the loan row already locked.
pub async fn backing_complete(
    tx: &mut Transaction<'_, Postgres>,
    loan_id: Uuid,
) -> Result<bool, E> {
    // The borrower's coins, if this loan has a leg on them. `pending` means
    // the lock has not been verified on chain yet — for either product.
    let coins_pending: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM public.xlm_collateral
          WHERE loan_id = $1 AND status = 'pending')",
    )
    .bind(loan_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| db_err(e, "collateral leg"))?;
    if coins_pending {
        return Ok(false);
    }

    let row: Option<(String, i64, i64)> = sqlx::query_as(
        "SELECT product, principal, borrower_cover_centavos
           FROM public.loans WHERE id = $1",
    )
    .bind(loan_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| db_err(e, "loan backing"))?;
    let (product, principal, cover) =
        row.ok_or((StatusCode::NOT_FOUND, "No such loan"))?;

    // Only a guarantor loan has a share somebody else carries.
    if product != "guarantor" {
        return Ok(true);
    }
    let gap = principal - cover;
    if gap <= 0 {
        return Ok(true);
    }

    let accepted: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(pledge_amount), 0)::BIGINT FROM public.loan_guarantors
          WHERE loan_id = $1 AND status = 'accepted'",
    )
    .bind(loan_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| db_err(e, "accepted pledges"))?;
    Ok(accepted >= gap)
}

/// Disburse, but only once every leg is in place. Returns whether it funded,
/// so the caller can tell the member which of the two they are still waiting
/// on. Callers must hold the loan row lock.
pub async fn try_disburse(
    tx: &mut Transaction<'_, Postgres>,
    loan_id: Uuid,
    borrower_id: Uuid,
    principal: i64,
    rate_bps: i32,
    term_months: i16,
) -> Result<bool, E> {
    if !backing_complete(tx, loan_id).await? {
        return Ok(false);
    }
    disburse(tx, loan_id, borrower_id, principal, rate_bps, term_months).await?;
    Ok(true)
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

    // Free cash, not raw cash: withdrawals promised and not yet paid out are
    // gone as far as new lending is concerned; so are other borrowers'
    // proceeds still waiting in their balances — theirs to withdraw at any
    // moment (050) — and the platform, reserve and recovery funds, which are
    // held rather than lent.
    let cash = free_cash(&mut **tx)
        .await
        .map_err(|e| db_err(e, "cash balance"))?
        - unwithdrawn_proceeds(&mut **tx)
            .await
            .map_err(|e| db_err(e, "proceeds waiting"))?
        - retained_funds(&mut **tx)
            .await
            .map_err(|e| db_err(e, "retained funds"))?;
    if cash < principal {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "The pool doesn't have enough liquidity right now — try a smaller amount or come back later",
        ));
    }

    // The depositor-visible side of funding. What the borrower backs with their
    // own locked deposit is their money on the line, not anyone else's: a
    // deposit-backed loan locks at least principal / LTV of it, so nobody
    // else's balance moves; an XLM loan has none, so the pool funds all of it;
    // a guarantor loan funds everything past the borrower's own deposit cover.
    // One rule, read off the lots themselves rather than the product name, so
    // it cannot drift from what apply actually froze. The rest is taken
    // pro-rata from every member's available balance and unlocks the same way
    // as principal comes back.
    let own_backing: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0)::BIGINT FROM public.deposits
          WHERE backing_loan = $1 AND badge = 'collateral'",
    )
    .bind(loan_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| db_err(e, "own deposit backing"))?;
    let pool_funded = domain::pool_funded_amount(principal, own_backing);
    lots::freeze_funding_pro_rata(tx, pool_funded, loan_id).await?;

    // The proceeds land in the borrower's pool balance (050): a withdrawable
    // lot, taken out through the same withdrawal every other peso uses. They
    // used to wait as a payout owed until the borrower pressed "send", which
    // only worked while the loan was active — so a loan repaid before anyone
    // pressed it left its proceeds owed with no way to claim them.
    let proceeds_lot: Uuid = sqlx::query_scalar(
        "INSERT INTO public.deposits (user_id, amount, badge, origin)
         VALUES ($1, $2, 'available', 'loan_proceeds') RETURNING id",
    )
    .bind(borrower_id)
    .bind(principal)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| db_err(e, "credit loan proceeds"))?;

    let now = Utc::now().timestamp();
    commit_event(
        tx,
        EventDraft {
            kind: "loan_disbursed",
            user_id: Some(borrower_id),
            loan_id: Some(loan_id),
            deposit_id: Some(proceeds_lot),
            rail_ref: None,
            payload: serde_json::json!({
                "principal": principal, "rate_bps": rate_bps, "term_months": term_months,
                "own_deposit_backing": own_backing, "pool_funded": pool_funded
            }),
            actor_id: Some(borrower_id),
        },
        // The pool is owed the principal, and owes the borrower a deposit of
        // the same amount. No `cash` moves: the pesos leave the platform's
        // balance only when the borrower withdraws them.
        &[
            Posting { account: "loans_receivable", amount: principal },
            Posting { account: "member_deposits", amount: -principal },
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
