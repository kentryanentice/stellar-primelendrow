//! The Stripe rail's HTTP surface — the mirror of `api::paypal`, plus the two
//! endpoints Stripe's model needs that PayPal's doesn't.
//!
//!   connect  — starts Express onboarding: creates the member's connected
//!              account if they don't have one, mints a single-use `state`
//!              bound to them, and hands back Stripe's onboarding URL.
//!   return   — Stripe redirects the member's browser back here; the engine
//!              asks Stripe what the account can actually do and records it.
//!   refresh  — where Stripe sends them when the onboarding link went stale.
//!   account  — what is linked right now, and unlinking it.
//!   checkout — creates the hosted payment session for a deposit or a
//!              repayment. No PayPal equivalent: on that rail the browser
//!              creates the order, which is precisely the weakness this
//!              removes (see `checkout.rs`).
//!   webhook  — the state Stripe changes on its own timetable. Never credits
//!              money; see `webhook.rs`.
//!
//! The member never types a destination. The only thing money is ever
//! addressed to is the connected account id Stripe itself issued for an
//! account the member onboarded on Stripe's own domain.

mod account;
mod checkout;
mod connect;
mod webhook;

pub use account::{disconnect, status};
pub use checkout::start as checkout;
pub use connect::{callback, refresh, start};
pub use webhook::handle as webhook;
