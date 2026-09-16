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

use crate::api::lending::intents;
use crate::api::lending::shared::validate_centavos;
use crate::api::users::shared::{E, require_verified_user};
use crate::infra::stripe;

#[derive(Deserialize)]
pub struct CheckoutInput {
    /// "deposit" or "repay" — what the money is for. Whitelisted at the door
    /// rather than trusted, same as `validate_product`.
    purpose: String,
    /// Whole centavos, for a deposit. Ignored for a repayment, whose amount
    /// the engine sets to exactly what's due.
    #[serde(default)]
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

    if !stripe::is_configured() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Stripe isn't configured on this deployment yet",
        ));
    }

    // Reserved before Stripe is asked for anything (041) — same rule as the
    // PayPal rail: a repayment is exactly what's due, a deposit must fit the
    // member's AML limits.
    //
    // Where the member lands afterwards, and what they are paying for. Both
    // are decided here from a whitelisted purpose — never assembled from
    // anything the client sent, or the success URL becomes an open redirect.
    // These must be real client routes. The app's catch-all redirects an
    // unknown path to /auth WITHOUT its query string, so a wrong path here
    // doesn't 404 visibly — it silently drops the session id, and a member
    // who has already paid is never credited.
    let (reserved, description, success_path, cancel_path) = match p.purpose.as_str() {
        "deposit" => {
            let amount = validate_centavos(p.amount)?;
            (
                intents::reserve_deposit(&pool, user_id, "stripe", amount).await?,
                "PrimeLendRow pool deposit".to_string(),
                "/lending".to_string(),
                "/lending".to_string(),
            )
        }
        "repay" => {
            let loan_id = p.loan_id.ok_or((
                StatusCode::UNPROCESSABLE_ENTITY,
                "Which loan is this repaying?",
            ))?;
            (
                intents::reserve_repay(&pool, user_id, "stripe", loan_id).await?,
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
    intents::close_superseded(&reserved.superseded).await;

    let session = match stripe::create_checkout_session(
        &user_id.to_string(),
        reserved.amount,
        &description,
        &p.purpose,
        p.loan_id.map(|id| id.to_string()).as_deref(),
        &success_path,
        &cancel_path,
        reserved.expires_at,
    )
    .await
    {
        Ok(session) => session,
        Err(m) => {
            intents::abandon(&pool, reserved.id).await;
            return Err((StatusCode::BAD_GATEWAY, m));
        }
    };
    // Superseded while Stripe was being called: close the page we just made
    // rather than hand out one that could still be paid.
    if !intents::open(&pool, reserved.id, &session.id).await? {
        let _ = stripe::expire_session(&session.id).await;
        return Err((StatusCode::CONFLICT, "A newer payment was started for this loan — use that one"));
    }

    tracing::info!(%user_id, amount = reserved.amount, purpose = %p.purpose, "stripe checkout session created");

    Ok(Json(CheckoutResponse {
        url: session.url,
        session_id: session.id,
    }))
}

#[derive(Deserialize)]
pub struct CancelInput {
    /// "deposit" or "repay" — which of the member's live pages to close.
    purpose: String,
}

#[derive(Serialize)]
pub struct CancelledResponse {
    pub cancelled: bool,
}

/// POST /stripe/checkout/cancel — the member came back from Stripe without
/// paying.
///
/// Nothing was charged, and nothing reaches the books: money only ever enters
/// through `/pool/deposit` or `/loans/repay` verifying a real payment. What
/// this does is close the checkout page with Stripe and release the record it
/// was holding — the same thing the PayPal rail does on cancel — so the
/// member's deposit limit is free again immediately rather than in an hour.
///
/// A page that turns out to have been paid is left alone, so it can still be
/// applied or refunded through the ordinary path.
pub async fn cancel(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(p): Json<CancelInput>,
) -> Result<Json<CancelledResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;
    let purpose = match p.purpose.as_str() {
        "deposit" => "deposit",
        "repay" => "repay",
        _ => return Err((StatusCode::UNPROCESSABLE_ENTITY, "Unknown payment purpose")),
    };
    intents::cancel_stripe(&pool, user_id, purpose).await?;
    Ok(Json(CancelledResponse { cancelled: true }))
}
