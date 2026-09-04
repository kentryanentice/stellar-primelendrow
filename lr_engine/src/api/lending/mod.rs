//! The lending pool: deposits, three loan products, guarantors, XLM
//! collateral. Structure follows the blueprint's layers, folded into the
//! codebase's existing per-feature module style:
//!
//!   domain   — PURE rules (pricing, caps, LTV, schedules, rounding)
//!   policy   — the versioned rulebook loader (D8)
//!   pricing  — the live XLM/PHP rate: many feeds in, one agreed number out
//!   ledger   — the ONE writer + balance reads (D9)
//!   lots     — badge moves on deposit lots, always under row locks
//!   shared   — error mapping + the single disburse routine
//!   the rest — one file per endpoint, same as api::wallets

mod actions;
mod admin;
mod admin_loans;
mod apply;
mod collateral;
mod custody;
mod default_loan;
mod deposit;
mod deposits_list;
mod domain;
mod guarantors;
pub(crate) mod ledger;
mod loans;
mod lots;
mod payments;
mod payout;
mod policy;
mod pool;
mod pricing;
mod quote;
mod recovery;
mod repay;
pub(crate) mod shared;
mod transactions;
mod withdraw;

pub use actions::{confirm as action_confirm, list as actions_list, prepare as action_prepare};
pub use admin::set_fx_rate;
pub use admin_loans::list as admin_loans;
pub use default_loan::declare as loan_default;
pub use apply::apply;
pub use collateral::confirm as collateral_confirm;
pub use custody::record as collateral_record;
pub use deposit::deposit;
pub use deposits_list::list as deposits_list;
pub use guarantors::{invites as guarantor_invites, respond as guarantor_respond};
pub use loans::{history as loans_history, list as loans_list};
pub use payments::list as payments_list;
pub use payout::{list as payouts_list, request as payout_request};
pub use pool::summary as pool_summary;
pub use quote::quote as loan_quote;
pub use repay::repay;
pub use transactions::list as transactions_list;
pub use withdraw::withdraw;
