//! POST /stripe/checkout — start a payment, and get the page to pay it on.
//!
//! This is the step the PayPal rail doesn't have, and the reason the Stripe
//! money-in path is the safer of the two.
//!
//! On PayPal, the *browser* creates the order: `PayPalButton.tsx` calls
//! `actions.order.create` with an amount, and the engine only sees an order id
//! afterwards. That is survivable because the engine re-reads what was
//! actually captured — but the order carries no claim about *who* it belongs
//! to, so the ownership check has to be "whoever presents the id".
//!
//! Here the engine creates the session, which means the engine decides the
//! amount, stamps the paying member's id into `client_reference_id`, and knows
//! before a peso moves which loan (if any) the payment is for. A member
//! confirming a deposit against somebody else's paid session is refused in
//! `capture_session`, because the session says whose it is.
//!
//! Nothing is written to the books here. A created session is an intention to
//! pay; the money exists only once `/pool/deposit` or `/loans/repay` verifies
//! it, exactly as with a PayPal order.

use axum::{Extension, Json, http::{HeaderMap, StatusCode}};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::lending::shared::validate_centavos;
use crate::api::users::shared::{E, require_verified_user};
use crate::infra::stripe;

#[derive(Deserialize)]
pub struct CheckoutInput {
    /// "deposit" or "repay" — what the money is for. Whitelisted at the door
    /// rather than trusted, same as `validate_product`.
    purpose: String,
    /// Whole centavos.
    amount: i64,
    /// Required for "repay", ignored otherwise.
    #[serde(default)]
    loan_id: Option<Uuid>,
}

#[derive(Serialize)]
pub struct CheckoutResponse {
    /// Stripe's hosted payment page. The frontend navigates the top-level
    /// window here — Stripe Checkout refuses to be framed.
    pub url: String,
    /// Echoed back so a caller can correlate its own logs; the engine never
    /// needs the client to send it, since Stripe puts it in the return URL.
    pub session_id: String,
}

pub async fn start(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(p): Json<CheckoutInput>,
) -> Result<Json<CheckoutResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;
    let mut amount = validate_centavos(p.amount)?;

    if !stripe::is_configured() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Stripe isn't configured on this deployment yet",
        ));
    }

    // Where the member lands afterwards, and what they are paying for. Both
    // are decided here from a whitelisted purpose — never assembled from
    // anything the client sent, or the success URL becomes an open redirect.
    let (description, success_path, cancel_path) = match p.purpose.as_str() {
        // These must be real client routes. The app's catch-all redirects an
        // unknown path to /auth WITHOUT its query string, so a wrong path here
        // doesn't 404 visibly — it silently drops the session id, and a member
        // who has already paid is never credited.
        "deposit" => (
            "PrimeLendRow pool deposit".to_string(),
            "/lending".to_string(),
            "/lending".to_string(),
        ),
        "repay" => {
            let loan_id = p.loan_id.ok_or((
                StatusCode::UNPROCESSABLE_ENTITY,
                "Which loan is this repaying?",
            ))?;

            // Checked before the member pays, not after. The authoritative
            // check still happens in `repay`, under the loan's row lock — this
            // one exists so nobody is sent to a payment page for a loan that
            // can't accept the money.
            //
            // Shared with the PayPal path rather than reimplemented: this used
            // to test `status != "active"` on its own, which refused a
            // *reopened default* outright and left settling borrowers unable to
            // pay by card at all.
            let payable = crate::api::lending::check_loan_payable(&pool, loan_id, user_id).await?;

            // A settlement is quoted, not chosen. Paying more than the arrears
            // does no harm — `settle` hands the excess straight back as a
            // deposit lot — but sending someone to a payment page for a number
            // larger than they owe, then returning most of it, is a confusing
            // way to take money. Clamp to what is actually outstanding.
            if payable.settling {
                amount = amount.min(payable.arrears);
            }

            (
                "PrimeLendRow loan repayment".to_string(),
                format!("/pay?loan={loan_id}"),
                format!("/pay?loan={loan_id}"),
            )
        }
        _ => {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                "Unknown payment purpose",
            ));
        }
    };

    let session = stripe::create_checkout_session(
        &user_id.to_string(),
        amount,
        &description,
        &p.purpose,
        p.loan_id.map(|id| id.to_string()).as_deref(),
        &success_path,
        &cancel_path,
    )
    .await
    .map_err(|m| (StatusCode::BAD_GATEWAY, m))?;

    tracing::info!(%user_id, amount, purpose = %p.purpose, "stripe checkout session created");

    Ok(Json(CheckoutResponse {
        url: session.url,
        session_id: session.id,
    }))
}
