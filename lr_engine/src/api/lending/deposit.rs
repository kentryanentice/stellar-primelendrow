//! POST /pool/deposit — PHP money-in via PayPal.
//!
//! The client sends nothing but the PayPal order id it just approved. The
//! engine captures the order server-side (infra::paypal, secret never leaves
//! the backend) and credits exactly what PayPal says was captured — the
//! client's screen never decides a centavo. A re-sent order id bounces off
//! the ledger's unique rail_ref (idempotent money-in, Lesson 9).

use axum::{Extension, Json, http::HeaderMap};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::ledger::{EventDraft, LedgerError, Posting, commit_event};
use super::policy;
use super::shared::{db_err, ledger_err};
use crate::api::users::shared::{E, require_verified_user};
use crate::infra::paypal;

#[derive(Deserialize)]
pub struct DepositInput {
    order_id: String,
}

#[derive(Serialize)]
pub struct DepositResponse {
    pub lot_id: Uuid,
    pub amount: i64,
    pub message: &'static str,
}

pub async fn deposit(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(p): Json<DepositInput>,
) -> Result<Json<DepositResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    let rules = policy::active(&pool).await?;

    // Network call happens BEFORE the transaction — no DB locks are ever
    // held across a round-trip to PayPal.
    let captured = paypal::capture_order(p.order_id.trim())
        .await
        .map_err(|m| (axum::http::StatusCode::UNPROCESSABLE_ENTITY, m))?;

    if captured.centavos < rules.params.min_deposit {
        // The money was really captured; refusing the lot would strand it.
        // This is a display-side floor — enforce it in the UI before order
        // creation, accept anything actually captured here, but log it.
        tracing::warn!(%user_id, centavos = captured.centavos, "deposit below policy minimum accepted");
    }

    let mut tx = pool.begin().await.map_err(|e| db_err(e, "begin deposit"))?;

    let lot_id: Uuid = sqlx::query_scalar(
        "INSERT INTO public.deposits (user_id, amount, badge) VALUES ($1, $2, 'available')
         RETURNING id",
    )
    .bind(user_id)
    .bind(captured.centavos)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| db_err(e, "insert lot"))?;

    let amount = captured.centavos;
    match commit_event(
        &mut tx,
        EventDraft {
            kind: "deposit_confirmed",
            user_id: Some(user_id),
            loan_id: None,
            deposit_id: Some(lot_id),
            rail_ref: Some(captured.capture_id),
            payload: serde_json::json!({ "rail": "paypal", "amount": amount }),
            actor_id: Some(user_id),
        },
        &[
            Posting { account: "cash", amount },
            Posting { account: "member_deposits", amount: -amount },
        ],
    )
    .await
    {
        Ok(_) => {}
        Err(LedgerError::DuplicateRail) => {
            // Whole transaction (including the freshly inserted lot) rolls
            // back — the first arrival of this capture already made the lot.
            return Err(ledger_err(LedgerError::DuplicateRail, "deposit"));
        }
        Err(e) => return Err(ledger_err(e, "deposit")),
    }

    tx.commit().await.map_err(|e| db_err(e, "commit deposit"))?;
    tracing::info!(%user_id, %lot_id, amount, "deposit confirmed");

    Ok(Json(DepositResponse {
        lot_id,
        amount,
        message: "Deposit received — it's in the pool and withdrawable until it funds a loan",
    }))
}
