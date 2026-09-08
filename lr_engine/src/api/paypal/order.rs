//! POST /paypal/order — creates the order a member is about to approve.
//!
//! This used to happen in the browser, through the Buttons SDK's
//! `actions.order.create`. That left the order carrying two things the engine
//! had no say in: the amount, and nothing at all about whose payment it was.
//! The second was the serious one — an order id is a bearer reference, so
//! anyone who came by one could present it to `/pool/deposit` and have the
//! money credited to their own account.
//!
//! Creating it here fixes both by construction. The engine decides the amount,
//! and stamps the caller's id into `custom_id`, which PayPal echoes back on
//! capture and `capture_order` checks. It is deliberately the same shape as
//! the Stripe rail's Checkout Session: the engine creates the thing being paid
//! for, and the client is handed nothing but a reference to approve.

use axum::{Extension, Json, http::{HeaderMap, StatusCode}};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::users::shared::{E, require_verified_user};
use crate::infra::paypal;

#[derive(Deserialize)]
pub struct OrderInput {
    /// Whole centavos. Checked here and sent to PayPal by the engine, so the
    /// page cannot ask for one amount and be charged another.
    amount: i64,
    /// `deposit` or `repay` — what the member is paying for. Only used to
    /// describe the order; what it actually settles is decided by which
    /// endpoint the resulting order id is later presented to.
    #[serde(default)]
    purpose: String,
    /// repay only: which loan, for the description.
    #[serde(default)]
    loan_id: Option<Uuid>,
}

#[derive(Serialize)]
pub struct OrderResponse {
    /// PayPal's order id, for the Buttons SDK to approve.
    pub order_id: String,
}

pub async fn create(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(p): Json<OrderInput>,
) -> Result<Json<OrderResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    if p.amount <= 0 {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Invalid amount"));
    }

    // Whitelisted, never interpolated from what the client sent — the
    // description is shown to the member on PayPal's own page.
    let description = match p.purpose.as_str() {
        "repay" => match p.loan_id {
            Some(loan_id) => format!("PrimeLendRow loan repayment {loan_id}"),
            None => "PrimeLendRow loan repayment".to_string(),
        },
        _ => "PrimeLendRow pool deposit".to_string(),
    };

    let order_id = paypal::create_order(&user_id.to_string(), p.amount, &description)
        .await
        .map_err(|m| (StatusCode::BAD_GATEWAY, m))?;

    tracing::info!(%user_id, amount = p.amount, purpose = %p.purpose, "paypal order created");

    Ok(Json(OrderResponse { order_id }))
}
