//! POST /pool/withdraw — take back 'available' deposit, and only that.
//!
//! The engine, not the UI, is what refuses frozen money: lots wearing
//! 'lent'/'collateral'/'pledged' simply aren't in the FOR UPDATE set this
//! handler consumes. The pool lock closes the race against a concurrent
//! disbursement deciding on the same cash.
//!
//! Rail note (029): this used to record the withdrawal and leave a note for
//! ops to pay by hand. It now rides the same PayPal Payouts rail as loan
//! proceeds (`payout.rs`), in the same three steps, for the same reason:
//!
//!   1. consume the lots and write the promise (`payout_payable`) and the
//!      `payouts` row in ONE transaction, and COMMIT — the row's primary key
//!      is the idempotency key PayPal will be given, so it must exist before
//!      any money can;
//!   2. call PayPal outside any transaction, holding no locks;
//!   3. record what PayPal said.
//!
//! The books therefore move in two beats rather than one. Requesting a
//! withdrawal no longer posts `cash`: the pesos are still in the platform's
//! PayPal balance at that moment, exactly as they are after a disbursement.
//! `cash` falls in `infra::payouts::settle`, against the transfer's own
//! reference, and only when PayPal confirms SUCCESS. Since `free_cash` reads
//! `cash + payout_payable`, a promised withdrawal is immediately unavailable
//! for new lending — the member's money can't be lent out from under them
//! while it is in flight.

use axum::{Extension, Json, http::{HeaderMap, StatusCode}};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::ledger::{EventDraft, Posting, commit_event, free_cash};
use super::lots;
use super::payout::{self, PayoutView};
use super::shared::{db_err, ledger_err, validate_centavos};
use crate::api::users::shared::{E, require_verified_user};

#[derive(Deserialize)]
pub struct WithdrawInput {
    /// Whole centavos.
    amount: i64,
}

#[derive(Serialize)]
pub struct WithdrawResponse {
    /// The transfer itself, in the same shape GET /payouts returns — the UI
    /// tracks a withdrawal exactly the way it tracks loan proceeds.
    pub payout: PayoutView,
    pub message: &'static str,
}

pub async fn withdraw(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(p): Json<WithdrawInput>,
) -> Result<Json<WithdrawResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;
    let amount = validate_centavos(p.amount)?;
    // Refused before a single lot is touched: a withdrawal we have nowhere to
    // send would consume the member's deposit into a promise that can never
    // be kept.
    let payer_id = payout::destination(&pool, user_id).await?;

    let mut tx = pool.begin().await.map_err(|e| db_err(e, "begin withdraw"))?;

    // Locks before decisions: pool first (serializes against disburse), then
    // the caller's own lots.
    sqlx::query("SELECT id FROM public.pool_control WHERE id = 1 FOR UPDATE")
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| db_err(e, "pool lock"))?;

    let my_lots = lots::lock_available_for_user(&mut tx, user_id).await?;
    let withdrawable: i64 = my_lots.iter().map(|l| l.amount).sum();
    if withdrawable < amount {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "That exceeds your withdrawable balance — deposits funding active loans stay locked until repayment",
        ));
    }

    // The books are the wall: even a correct lot sum can't overdraw actual
    // cash (loans out are cash gone until repaid).
    // Free cash: loan proceeds and earlier withdrawals already promised but
    // not yet paid out are not available to cover this one, even though they
    // haven't left the platform's PayPal balance yet (028, 029).
    let cash = free_cash(&mut *tx)
        .await
        .map_err(|e| db_err(e, "cash balance"))?;
    if cash < amount {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "The pool can't cover this right now — most of it is out on loans. Try again after repayments arrive",
        ));
    }

    // Consume FIFO: whole lots go, the partial tail shrinks in place. The
    // withdrawal event below is the auditable record of where they went.
    let now = Utc::now().timestamp();
    let mut remaining = amount;
    for lot in &my_lots {
        if remaining == 0 {
            break;
        }
        if lot.amount <= remaining {
            sqlx::query("DELETE FROM public.deposits WHERE id = $1")
                .bind(lot.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| db_err(e, "consume lot"))?;
            remaining -= lot.amount;
        } else {
            sqlx::query("UPDATE public.deposits SET amount = amount - $1, updated_at = $2 WHERE id = $3")
                .bind(remaining)
                .bind(now)
                .bind(lot.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| db_err(e, "shrink lot"))?;
            remaining = 0;
        }
    }

    // The destination is pinned to the row now, for the same reason it is on
    // a loan payout: relinking a PayPal account later must not redirect a
    // transfer that is already in flight.
    let payout_id: Uuid = sqlx::query_scalar(
        "INSERT INTO public.payouts (user_id, loan_id, kind, amount, payer_id)
         VALUES ($1, NULL, 'deposit_withdrawal', $2, $3)
         RETURNING id",
    )
    .bind(user_id)
    .bind(amount)
    .bind(&payer_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| db_err(e, "insert withdrawal payout"))?;

    commit_event(
        &mut tx,
        EventDraft {
            kind: "withdrawal_confirmed",
            user_id: Some(user_id),
            loan_id: None,
            deposit_id: None,
            rail_ref: None,
            payload: serde_json::json!({
                "amount": amount, "payout_id": payout_id, "rail": "paypal_payouts",
            }),
            actor_id: Some(user_id),
        },
        // The member's deposit claim is gone and the pool now OWES them the
        // pesos. `cash` is untouched until the transfer actually settles.
        &[
            Posting { account: "member_deposits", amount },
            Posting { account: "payout_payable", amount: -amount },
        ],
    )
    .await
    .map_err(|e| ledger_err(e, "withdrawal"))?;

    // Committed BEFORE the network call: the idempotency key has to survive a
    // crash mid-request, or a retry would mint a new one and PayPal would have
    // no way to recognise the duplicate.
    tx.commit().await.map_err(|e| db_err(e, "commit withdraw"))?;

    let (status, message) =
        payout::submit(&pool, payout_id, &payer_id, amount, "PrimeLendRow withdrawal").await;
    let payout = payout::read_one(&pool, payout_id, user_id).await?;
    tracing::info!(%user_id, %payout_id, amount, status, "withdrawal confirmed");

    Ok(Json(WithdrawResponse { payout, message }))
}
