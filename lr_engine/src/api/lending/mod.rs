//! The lending pool: deposits, three loan products, guarantors, XLM
//! collateral. Structure follows the blueprint's layers, folded into the
//! codebase's existing per-feature module style:
//!
//!   domain   — PURE rules (pricing, caps, LTV, schedules, rounding)
//!   policy   — the versioned rulebook loader (D8)
//!   ledger   — the ONE writer + balance reads (D9)
//!   lots     — badge moves on deposit lots, always under row locks
//!   shared   — error mapping + the single disburse routine
//!   the rest — one file per endpoint, same as api::wallets

mod admin;
mod apply;
mod collateral;
mod deposit;
mod deposits_list;
mod domain;
mod guarantors;
mod ledger;
mod loans;
mod lots;
mod payments;
mod policy;
mod pool;
mod quote;
mod repay;
mod shared;
mod withdraw;

pub use admin::set_fx_rate;
pub use apply::apply;
pub use collateral::confirm as collateral_confirm;
pub use deposit::deposit;
pub use deposits_list::list as deposits_list;
pub use guarantors::{invites as guarantor_invites, respond as guarantor_respond};
pub use loans::{history as loans_history, list as loans_list};
pub use payments::list as payments_list;
pub use pool::summary as pool_summary;
pub use quote::quote as loan_quote;
pub use repay::repay;
pub use withdraw::withdraw;
