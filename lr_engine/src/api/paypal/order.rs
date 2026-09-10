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
//!
//! Being the same shape means answering the same questions, and a repayment
//! asks one more than a deposit does: *can this loan take money right now?*
//! `check_loan_payable` answers it for both rails, here and in
//! `api::stripe::checkout`, so neither can end up offering a payment the other
//! would refuse.

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

    let mut amount = p.amount;

    // Whitelisted, never interpolated from what the client sent — the
    // description is shown to the member on PayPal's own page.
    let description = match p.purpose.as_str() {
        "repay" => {
            // The loan is checked HERE, before PayPal is asked for anything.
            //
            // This endpoint used to take `loan_id` as decoration for the
            // description and check nothing at all — not that the loan existed,
            // not that it belonged to the caller, not that it could accept
            // money. `/loans/repay` would refuse a bad one later, but only
            // after the member had walked through PayPal's whole approval
            // sheet, which is a rotten way to say "you didn't owe this".
            //
            // It is also what let a fully-settled loan keep offering to be
            // paid: the Stripe rail already refused that at checkout, and this
            // one didn't, so the two rails disagreed about the same loan.
            let loan_id = p.loan_id.ok_or((
                StatusCode::UNPROCESSABLE_ENTITY,
                "Which loan is this repaying?",
            ))?;
            let payable =
                crate::api::lending::check_loan_payable(&pool, loan_id, user_id).await?;

            // Same clamp the Stripe checkout applies: paying more than the
            // arrears is harmless — the excess comes back as a deposit lot —
            // but showing someone a PayPal sheet for more than they owe and
            // then returning most of it is a confusing way to take money.
            if payable.settling {
                amount = amount.min(payable.arrears);
            }

            format!("PrimeLendRow loan repayment {loan_id}")
        }
        _ => "PrimeLendRow pool deposit".to_string(),
    };

    let order_id = paypal::create_order(&user_id.to_string(), amount, &description)
        .await
        .map_err(|m| (StatusCode::BAD_GATEWAY, m))?;

    tracing::info!(%user_id, amount, purpose = %p.purpose, "paypal order created");

    Ok(Json(OrderResponse { order_id }))
}
