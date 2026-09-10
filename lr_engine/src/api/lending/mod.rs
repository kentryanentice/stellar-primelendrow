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
//!   admin    — the operator's endpoints, all `require_admin` (see admin/mod)
//!   the rest — one file per endpoint, same as api::wallets

pub mod admin;
mod apply;
mod cancel;
mod collateral;
mod custody;
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
mod rails;
mod recovery;
mod repay;
pub(crate) mod shared;
mod transactions;
mod withdraw;

// The operator's surface, re-exported flat so `routes::api_routes` keeps
// naming handlers the way it always has — the folder is for the people reading
// the code, not a reshuffle of the route table.
pub use admin::actions::{confirm as action_confirm, list as actions_list, prepare as action_prepare};
pub use admin::default::declare as loan_default;
pub use admin::fx::set_fx_rate;
pub use admin::loans::list as admin_loans;
// Settling a defaulted loan (033): reopening it for payment and accepting it as
// settled are two separate admin decisions, so they are two separate handlers.
pub use admin::reconcile::{mark_paid as loan_mark_paid, reopen as loan_reopen};
// "May this loan take a payment right now?" — asked by every entry point
// BEFORE the provider is charged, so a refusal costs the borrower nothing.
// Exported because the Stripe checkout starts a payment from outside `lending`.
pub(crate) use admin::reconcile::check_payable as check_loan_payable;
pub use apply::apply;
pub use cancel::cancel as loan_cancel;
pub use collateral::confirm as collateral_confirm;
pub use custody::record as collateral_record;
pub use deposit::deposit;
// The Stripe webhook credits through the same routine the redirect does, so a
// member who never comes back to the app is still credited.
pub(crate) use deposit::credit as credit_deposit;
pub use deposits_list::list as deposits_list;
pub use guarantors::{invites as guarantor_invites, respond as guarantor_respond};
pub use loans::{history as loans_history, list as loans_list};
pub use payments::list as payments_list;
pub use payout::{list as payouts_list, request as payout_request};
// The retry sweep in `infra::payouts` submits over whichever rail a row was
// created for, and this is the one description of how to do that — exported
// rather than reimplemented so a request-time submission and a retry can never
// disagree about which provider, or which idempotency key, a payout uses.
pub(crate) use payout::submit_to as submit_payout;
pub use pool::summary as pool_summary;
pub use quote::quote as loan_quote;
pub use repay::repay;
pub use transactions::list as transactions_list;
pub use withdraw::withdraw;
// The sweep calls this whenever a payout reaches a terminal state it never
// arrived from. It no-ops for loan proceeds, so both call sites can call it
// unconditionally rather than each deciding what a failure means.
pub(crate) use withdraw::refund_if_failed as refund_failed_withdrawal;
