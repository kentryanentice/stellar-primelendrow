//! The operator's side of the lending pool.
//!
//! Everything in here is `require_admin` inside the handler, and that is the
//! reason the folder exists: an admin-only endpoint that drifts into the
//! borrower files is one a reviewer has to *notice* is privileged. Here the
//! path says it, and "does every file in this directory check
//! `require_admin`?" is a question you can answer by reading five files
//! instead of twenty-seven.
//!
//!   fx        — pin the XLM/PHP rate used for collateral valuation
//!   loans     — the loan book: every loan, its schedule, collateral,
//!               recoveries and movements, paginated
//!   default   — declare a loan defaulted, and run the recovery waterfall
//!   reconcile — the way back from a default (033): reopen for settlement,
//!               then accept it as settled
//!   actions   — the vault outbox: prepare and confirm the on-chain movements
//!               a default or a repayment queued for the admin's own key
//!
//! **The route table is still not the authorization.** Being in this folder
//! changes nothing at runtime — every handler below calls `require_admin`
//! itself, exactly as it did when these files sat one directory up. Moving a
//! file must never be what makes it safe.
//!
//! What deliberately stayed behind in `lending/`:
//!
//!   * `recovery` — the default waterfall. Only admin paths call it, but it is
//!     domain logic that moves money, not an HTTP surface, so it belongs on the
//!     same shelf as `ledger` and `lots`.
//!   * `shared`, `ledger`, `lots`, `domain`, `policy` — used by both sides.
//!
//! Nothing had to be made more public to allow the move. A child module can
//! already see its ancestors' private items, so `lending`'s private `mod lots`,
//! `mod recovery` and friends stay reachable from in here — the imports are
//! spelled `crate::api::lending::lots` rather than `super::lots` only because
//! `super` now means this folder, and the explicit path says which shelf the
//! thing actually lives on.
//!
//! The traffic in the other direction is the part worth keeping an eye on:
//! `repay` reaches into `reconcile` for the settlement split, because a
//! settlement is admin-initiated but the payment lands on the borrower's own
//! rail. Those two items (`arrears`, `settle`) are `pub(in crate::api::lending)`
//! rather than `pub` — visible to the lending module that needs them and to
//! nothing beyond it, so the folder boundary stays real.

pub mod actions;
pub mod default;
pub mod fx;
pub mod loans;
pub mod reconcile;
