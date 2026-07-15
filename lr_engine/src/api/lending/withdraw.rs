//! POST /pool/withdraw — take back 'available' deposit, and only that.
//!
//! The engine, not the UI, is what refuses frozen money: lots wearing
//! 'lent'/'collateral'/'pledged' simply aren't in the FOR UPDATE set this
//! handler consumes. The pool lock closes the race against a concurrent
//! disbursement deciding on the same cash.
//!
//! Rail note (D7): this records the withdrawal in the books immediately;
//! the actual PayPal payout is executed by ops from the withdrawal events
//! (manual rail) until an automated payout rail is wired in.

use axum::{Extension, Json, http::{HeaderMap, StatusCode}};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use super::ledger::{EventDraft, Posting, account_balance, commit_event};
use super::lots;
use super::shared::{db_err, ledger_err, validate_centavos};
use crate::api::users::shared::{E, require_verified_user};

#[derive(Deserialize)]
pub struct WithdrawInput {
    /// Whole centavos.
    amount: i64,
}

#[derive(Serialize)]
pub struct WithdrawResponse {
    pub amount: i64,
    pub message: &'static str,
}

pub async fn withdraw(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(p): Json<WithdrawInput>,
) -> Result<Json<WithdrawResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;
    let amount = validate_centavos(p.amount)?;

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
    let cash = account_balance(&mut *tx, "cash")
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

    commit_event(
        &mut tx,
        EventDraft {
            kind: "withdrawal_confirmed",
            user_id: Some(user_id),
            loan_id: None,
            deposit_id: None,
            rail_ref: None,
            payload: serde_json::json!({ "amount": amount, "rail": "paypal_manual" }),
            actor_id: Some(user_id),
        },
        &[
            Posting { account: "member_deposits", amount },
            Posting { account: "cash", amount: -amount },
        ],
    )
    .await
    .map_err(|e| ledger_err(e, "withdrawal"))?;

    tx.commit().await.map_err(|e| db_err(e, "commit withdraw"))?;
    tracing::info!(%user_id, amount, "withdrawal confirmed");

    Ok(Json(WithdrawResponse {
        amount,
        message: "Withdrawal recorded — the payout to your PayPal is on its way",
    }))
}
