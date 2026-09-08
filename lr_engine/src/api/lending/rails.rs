//! Which rail the money came in on, and which one it goes out on.
//!
//! Two providers carry pesos now. The handlers that decide what the money
//! *means* — a deposit lot, a repayment allocation, a withdrawal promise —
//! must not also be the place that knows how each provider is talked to, or
//! every one of them grows a second copy of the same branch. So the choice is
//! made exactly twice: here for money in, and in `payout::destination` for
//! money out.
//!
//! Money in stays **pull-based on both rails**: the member tells the engine
//! they paid, and the engine asks the provider what actually happened. The
//! client's number is never the number. What differs is only which reference
//! it presents.

use axum::http::StatusCode;
use serde::Deserialize;
use uuid::Uuid;

use crate::api::users::shared::E;
// Re-exported: `deposit::credit` takes one of these, and the Stripe webhook
// builds one without going through `capture` below.
pub use crate::infra::rails::CapturedPayment;
use crate::infra::{paypal, stripe};

/// The reference a member presents to say "I paid". Exactly one of these is
/// expected; which one it is names the rail.
///
/// Flattened into the deposit and repay request bodies, so an existing client
/// that only knows `order_id` keeps working untouched — adding the Stripe rail
/// did not change the PayPal one.
#[derive(Deserialize)]
pub struct PaymentRef {
    /// PayPal: an approved order id from the Buttons flow.
    #[serde(default)]
    pub order_id: Option<String>,
    /// Stripe: a Checkout Session id from the redirect back.
    #[serde(default)]
    pub session_id: Option<String>,
}

/// A verified payment, and which rail verified it.
pub struct Captured {
    /// "paypal" or "stripe" — recorded in the ledger event's payload so the
    /// books say how each peso arrived.
    pub rail: &'static str,
    pub payment: CapturedPayment,
}

/// Verifies a payment with whichever provider it belongs to.
///
/// `user_id` is the member claiming it, and both rails now enforce that claim
/// rather than only Stripe. A Checkout Session records whose it is in
/// `client_reference_id`; a PayPal order records it in `custom_id`, stamped by
/// `paypal::create_order` when the engine — not the browser — creates the
/// order. Either way the reference alone proves nothing: the provider is asked
/// who it belongs to, and a mismatch is refused.
///
/// A PayPal order created before the engine started stamping carries no owner
/// and is refused outright. Those are true bearer references, and there is no
/// safe way to decide after the fact whose money they were.
///
/// Never called inside a database transaction: this is a network round trip,
/// and no lock is ever held across one.
pub async fn capture(p: &PaymentRef, user_id: Uuid) -> Result<Captured, E> {
    match (
        p.order_id.as_deref().map(str::trim).filter(|s| !s.is_empty()),
        p.session_id.as_deref().map(str::trim).filter(|s| !s.is_empty()),
    ) {
        // Both would mean the caller is unsure what it paid with, and guessing
        // between them risks crediting one payment while another stands open.
        (Some(_), Some(_)) => Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "Send either a PayPal order or a Stripe session, not both",
        )),
        (Some(order_id), None) => {
            let payment = paypal::capture_order(order_id, &user_id.to_string())
                .await
                .map_err(|m| (StatusCode::UNPROCESSABLE_ENTITY, m))?;
            Ok(Captured { rail: "paypal", payment })
        }
        (None, Some(session_id)) => {
            let payment = stripe::capture_session(session_id, &user_id.to_string())
                .await
                .map_err(|m| (StatusCode::UNPROCESSABLE_ENTITY, m))?;
            Ok(Captured { rail: "stripe", payment })
        }
        (None, None) => Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "No payment reference — nothing to confirm",
        )),
    }
}
