//! The default recovery waterfall (ARCHITECTURE §5.9).
//!
//! ```text
//! 1. borrower deposit   2. borrower XLM (seize)   3. guarantor deposit   4. guarantor XLM (seize)
//! ```
//!
//! The order is the promise: a guarantor is charged only after the borrower's
//! own money and coins are gone, and ordinary savers' deposits are never
//! touched at all — the `lent` badge is deliberately absent from every
//! selector below.
//!
//! **Why this is resumable rather than one pass.** Step 2 cannot complete
//! inside the request that declares the default. Seizing coins is an on-chain
//! movement the vault admin has to sign, and how much debt those coins cover
//! is decided by the price the CONTRACT checks at seizure time — not by any
//! number the engine could pick beforehand. So `advance` runs the waterfall as
//! far as the facts allow and stops at a locked position; the confirm handler
//! records what the chain actually moved and calls it again. A default with no
//! XLM behind it runs straight through on the first call.
//!
//! Every step that recovers anything writes two things: a balanced ledger
//! posting (the books) and a `loan_recoveries` row (the audit trail — who
//! paid, in what order, how much). Nothing here computes a peso value for
//! coins; that number arrives from the chain.

use axum::http::StatusCode;
use chrono::Utc;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use super::ledger::{EventDraft, Posting, commit_event};
use super::lots;
use super::shared::{db_err, ledger_err};
use crate::api::users::shared::E;

/// Where the waterfall got to. `awaiting_seizure` is the one state that needs
/// something outside the engine to happen next.
pub struct Progress {
    /// Centavos of principal still uncovered.
    pub shortfall: i64,
    /// True when a locked XLM position is standing between the borrower's
    /// deposits and the guarantors — recovery pauses here rather than
    /// charging a guarantor for a debt the coins may yet cover.
    pub awaiting_seizure: bool,
}

/// Applies `amount` of recovered value to the loan: the books move, the
/// outstanding principal falls, and the step is recorded.
///
/// `posting` is the account the value came FROM, in the ledger's terms —
/// `member_deposits` when a member's deposit was taken (their liability
/// shrinks), `treasury_assets` when seized coins landed in the treasury,
/// `reserve_fund` when the pool ate the rest. Every one of them is paired
/// against `loans_receivable`: the debt is what recovery destroys.
#[allow(clippy::too_many_arguments)]
async fn apply(
    tx: &mut Transaction<'_, Postgres>,
    loan_id: Uuid,
    step: i16,
    source: &'static str,
    account: &'static str,
    user_id: Option<Uuid>,
    amount: i64,
    stroops: Option<i64>,
    actor_id: Uuid,
) -> Result<(), E> {
    if amount <= 0 {
        return Ok(());
    }

    let event_id = commit_event(
        tx,
        EventDraft {
            kind: "default_recovery",
            user_id,
            loan_id: Some(loan_id),
            deposit_id: None,
            rail_ref: None,
            payload: serde_json::json!({
                "step": step, "source": source, "amount": amount, "stroops": stroops,
            }),
            actor_id: Some(actor_id),
        },
        &[
            Posting { account, amount },
            Posting { account: "loans_receivable", amount: -amount },
        ],
    )
    .await
    .map_err(|e| ledger_err(e, "default_recovery"))?;

    sqlx::query(
        "INSERT INTO public.loan_recoveries (loan_id, step, source, user_id, amount, stroops, event_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(loan_id)
    .bind(step)
    .bind(source)
    .bind(user_id)
    .bind(amount)
    .bind(stroops)
    .bind(event_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| db_err(e, "insert recovery"))?;

    // The debt is the thing being paid down; GREATEST keeps the column's
    // `>= 0` check honest against a final step that recovers more than is
    // outstanding (a seizure can overshoot — the coins are worth what they
    // are worth).
    sqlx::query(
        "UPDATE public.loans
            SET principal_outstanding = GREATEST(principal_outstanding - $1, 0), updated_at = $2
          WHERE id = $3",
    )
    .bind(amount)
    .bind(Utc::now().timestamp())
    .bind(loan_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| db_err(e, "write down loan"))?;

    Ok(())
}

/// Records what an on-chain seizure actually recovered. Called by the confirm
/// handler once Horizon has been asked, never before: until the coins have
/// moved there is nothing to record.
///
/// **The surplus is the normal case, not an edge case.** The vault demands
/// 120% coverage, so seized coins are usually worth more than the debt they
/// stood behind — and the contract sends all of them, because splitting a
/// position on-chain isn't something it does. Keeping the excess would be the
/// platform helping itself to the difference between a borrower's collateral
/// and their debt, which is not what collateral is. So the debt takes what it
/// is owed and the remainder goes back to the borrower as pool balance: a real
/// deposit lot, withdrawable like any other, because a liability with no lot
/// behind it would show up nowhere on their balance.
pub async fn record_seizure(
    tx: &mut Transaction<'_, Postgres>,
    loan_id: Uuid,
    borrower_id: Uuid,
    stroops: i64,
    value_centavos: i64,
    actor_id: Uuid,
) -> Result<(), E> {
    let outstanding: i64 = sqlx::query_scalar(
        "SELECT principal_outstanding FROM public.loans WHERE id = $1 FOR UPDATE",
    )
    .bind(loan_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| db_err(e, "read outstanding"))?;

    let applied = value_centavos.min(outstanding);
    let surplus = value_centavos - applied;

    if applied > 0 {
        apply(
            tx, loan_id, 2, "borrower_xlm", "treasury_assets",
            Some(borrower_id), applied, Some(stroops), actor_id,
        )
        .await?;
    }

    if surplus > 0 {
        let lot_id: Uuid = sqlx::query_scalar(
            "INSERT INTO public.deposits (user_id, amount, badge) VALUES ($1, $2, 'available')
             RETURNING id",
        )
        .bind(borrower_id)
        .bind(surplus)
        .fetch_one(&mut **tx)
        .await
        .map_err(|e| db_err(e, "return seizure surplus"))?;

        // The treasury holds the coins; the borrower is owed what they were
        // worth beyond the debt. Not a recovery step — nothing was recovered
        // here, it is the opposite — so it gets no `loan_recoveries` row.
        commit_event(
            tx,
            EventDraft {
                kind: "seizure_surplus_returned",
                user_id: Some(borrower_id),
                loan_id: Some(loan_id),
                deposit_id: Some(lot_id),
                rail_ref: None,
                payload: serde_json::json!({
                    "seized_value": value_centavos, "applied": applied, "surplus": surplus,
                    "stroops": stroops,
                }),
                actor_id: Some(actor_id),
            },
            &[
                Posting { account: "treasury_assets", amount: surplus },
                Posting { account: "member_deposits", amount: -surplus },
            ],
        )
        .await
        .map_err(|e| ledger_err(e, "seizure_surplus_returned"))?;
    }

    Ok(())
}

/// Runs the waterfall as far as the facts currently allow.
pub async fn advance(
    tx: &mut Transaction<'_, Postgres>,
    loan_id: Uuid,
    actor_id: Uuid,
) -> Result<Progress, E> {
    let loan: Option<(Uuid, i64, String)> = sqlx::query_as(
        "SELECT borrower_id, principal_outstanding, status FROM public.loans
          WHERE id = $1 FOR UPDATE",
    )
    .bind(loan_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| db_err(e, "lock defaulted loan"))?;
    let (borrower_id, mut shortfall, status) =
        loan.ok_or((StatusCode::NOT_FOUND, "No such loan"))?;
    if status != "defaulted" {
        return Err((StatusCode::CONFLICT, "This loan is not in default"));
    }

    // ---- step 1: the borrower's own money ---------------------------------
    // Their collateral for this loan first, then anything else of theirs
    // sitting withdrawable. Both are the defaulter's own deposit, and taking
    // it before a guarantor's is the whole point of the ordering.
    if shortfall > 0 {
        let backing = lots::lock_backing_lots(tx, loan_id, "collateral").await?;
        let taken = lots::seize_lots(tx, &backing, shortfall).await?;
        for (user_id, amount) in taken {
            apply(tx, loan_id, 1, "borrower_deposit", "member_deposits", Some(user_id), amount, None, actor_id).await?;
            shortfall -= amount;
        }
    }
    if shortfall > 0 {
        let free = lots::lock_available_for_user(tx, borrower_id).await?;
        let taken = lots::seize_lots(tx, &free, shortfall).await?;
        for (user_id, amount) in taken {
            apply(tx, loan_id, 1, "borrower_deposit", "member_deposits", Some(user_id), amount, None, actor_id).await?;
            shortfall -= amount;
        }
    }

    // ---- step 2: the borrower's coins -------------------------------------
    // Not recoverable from here. A position still `locked` means the seizure
    // hasn't happened on-chain, so the waterfall pauses: charging a guarantor
    // now would take their money for a debt the coins are about to cover.
    let locked_position: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM public.xlm_collateral WHERE loan_id = $1 AND status = 'locked'",
    )
    .bind(loan_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| db_err(e, "collateral position"))?;
    if locked_position.is_some() && shortfall > 0 {
        return Ok(Progress { shortfall, awaiting_seizure: true });
    }

    // ---- step 3: the guarantors -------------------------------------------
    // Pledged lots only. A guarantor who pledged ₱5,000 against a ₱1,000
    // shortfall loses ₱1,000 — seize_lots stops at the limit, so nobody is
    // charged for more of the debt than is left.
    if shortfall > 0 {
        let pledged = lots::lock_backing_lots(tx, loan_id, "pledged").await?;
        let taken = lots::seize_lots(tx, &pledged, shortfall).await?;
        for (user_id, amount) in taken {
            apply(tx, loan_id, 3, "guarantor_deposit", "member_deposits", Some(user_id), amount, None, actor_id).await?;
            shortfall -= amount;
        }
    }

    // ---- step 4: the guarantors' coins ------------------------------------
    // Not implementable against the current data model, and deliberately not
    // faked: `xlm_collateral` is one row per loan owned by the BORROWER
    // (`loan_id` is UNIQUE), so there is nowhere a guarantor's XLM could be
    // recorded. Guarantors back loans with pledged deposits only. Adding the
    // step means giving guarantors a collateral position of their own first.

    // ---- what nobody covered ----------------------------------------------
    // The pool absorbs it. Booking the loss is not optional: leaving
    // `loans_receivable` standing against a settled loan would overstate the
    // pool's assets by exactly the amount it just lost.
    if shortfall > 0 {
        apply(tx, loan_id, 4, "reserve_fund", "reserve_fund", None, shortfall, None, actor_id).await?;
        shortfall = 0;
    }

    // Settled. The savers who funded it are made whole by the postings above,
    // so their lots go back to withdrawable — the loan is over either way.
    lots::release_loan_lots(tx, loan_id, &["lent"]).await?;

    let now = Utc::now().timestamp();
    sqlx::query(
        "UPDATE public.loan_guarantors SET status = 'seized', updated_at = $1
          WHERE loan_id = $2 AND status = 'accepted'",
    )
    .bind(now)
    .bind(loan_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| db_err(e, "seize guarantors"))?;

    // Status stays 'defaulted' — that is the loan's outcome and a credit fact
    // about the borrower. `closed_at` records that recovery is finished, which
    // is a different question from how it ended.
    sqlx::query("UPDATE public.loans SET closed_at = $1, updated_at = $1 WHERE id = $2")
        .bind(now)
        .bind(loan_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| db_err(e, "close defaulted loan"))?;

    Ok(Progress { shortfall, awaiting_seizure: false })
}
