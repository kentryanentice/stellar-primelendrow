//! Connecting a member's own PayPal account, so the platform can pay them.
//!
//!   connect  — starts "Log in with PayPal": mints a single-use `state` bound
//!              to the caller and hands back PayPal's authorization URL.
//!   callback — PayPal redirects the member's browser back here with a code;
//!              the engine exchanges it server-side and stores the payer id.
//!   account  — what is linked right now, and unlinking it.
//!
//! The member never types a destination. The only thing money is ever
//! addressed to is the payer id PayPal itself returned for an account the
//! member authenticated against on PayPal's own domain.

mod account;
mod connect;

pub use account::{disconnect, status};
pub use connect::{callback, start};
