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
//! Being the same shape means answering the same questions, through the same
//! code: `lending::intents` reserves the payment for both rails (041). A
//! repayment is exactly what's due on the loan, a deposit must fit the member's
//! AML limits, and the order can only ever be confirmed as what it was created
//! for.

use axum::{Extension, Json, http::{HeaderMap, StatusCode}};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::lending::intents;
use crate::api::users::shared::{E, require_verified_user};
use crate::infra::paypal;

#[derive(Deserialize)]
pub struct OrderInput {
    /// Whole centavos, for a deposit. Ignored for a repayment, whose amount
    /// the engine sets to exactly what's due.
    #[serde(default)]
    amount: i64,
    /// `deposit` or `repay` — what the member is paying for. Recorded on the
    /// payment intent (041), so the order can only ever be confirmed as that.
    #[serde(default)]
    purpose: String,
    /// repay only: which loan.
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

    // The payment is reserved before PayPal is asked for anything (041): a
    // repayment's amount is set to exactly what's due — whatever the page
    // sent — and a deposit is checked against the member's AML limits. The
    // page's `amount` only matters for a deposit.
    //
    // Whitelisted, never interpolated from what the client sent — the
    // description is shown to the member on PayPal's own page.
    let (reserved, description) = match p.purpose.as_str() {
        "repay" => {
            let loan_id = p.loan_id.ok_or((
                StatusCode::UNPROCESSABLE_ENTITY,
                "Which loan is this repaying?",
            ))?;
            (
                intents::reserve_repay(&pool, user_id, "paypal", loan_id).await?,
                format!("PrimeLendRow loan repayment {loan_id}"),
            )
        }
        "deposit" => {
            if p.amount <= 0 {
                return Err((StatusCode::UNPROCESSABLE_ENTITY, "Invalid amount"));
            }
            (
                intents::reserve_deposit(&pool, user_id, "paypal", p.amount).await?,
                "PrimeLendRow pool deposit".to_string(),
            )
        }
        _ => return Err((StatusCode::UNPROCESSABLE_ENTITY, "Unknown payment purpose")),
    };
    intents::close_superseded(&reserved.superseded).await;

    let order_id = match paypal::create_order(&user_id.to_string(), reserved.amount, &description).await {
        Ok(id) => id,
        Err(m) => {
            intents::abandon(&pool, reserved.id).await;
            return Err((StatusCode::BAD_GATEWAY, m));
        }
    };
    // Superseded while PayPal was being called: the order is simply never
    // captured (capture only follows a claim), so there is nothing to undo.
    if !intents::open(&pool, reserved.id, &order_id).await? {
        return Err((StatusCode::CONFLICT, "A newer payment was started for this loan — use that one"));
    }

    tracing::info!(%user_id, amount = reserved.amount, purpose = %p.purpose, "paypal order created");

    Ok(Json(OrderResponse { order_id }))
}
