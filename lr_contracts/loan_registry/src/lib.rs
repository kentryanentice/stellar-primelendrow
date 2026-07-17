//! On-chain loan registry for PrimeLendRow.
//!
//! A tamper-evident, engine-authored mirror of the `loans` table
//! (migration 020). It does **not** move money — it records each loan's
//! pinned terms and status transitions on Stellar, keyed by the loan's UUID
//! (16 bytes), so the life of a loan is auditable on a ledger nobody can edit.
//!
//! Same trust model as the collateral vault: **only the admin (the engine's
//! Stellar account) can write.** Reads are open.
//!
//! Two rules from the engine are enforced here as hard contract invariants:
//!   * a loan's price (`rate_bps`) and rulebook (`policy_version`) are pinned
//!     at birth and never rewritten (D8);
//!   * a borrower may hold at most **one** open (pending or active) loan at a
//!     time — the same wall as the engine's partial unique index.
//!
//! Amounts are whole centavos, mirroring the engine's `BIGINT`.

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, Address, BytesN, Env,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InvalidAmount = 3,
    InvalidRate = 4,   // rate_bps outside 1..=2000 (monthly)
    InvalidTerm = 5,   // term outside 3..=12 months
    LoanExists = 6,
    LoanNotFound = 7,
    BorrowerHasOpenLoan = 8,
    NotPending = 9,          // transition needs a pending loan
    NotActive = 10,          // transition needs an active loan
    OutstandingRemains = 11, // cannot close with principal still owed
    ReduceExceedsOutstanding = 12,
}

/// The three products, mirroring the `loans.product` CHECK.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Product {
    DepositBacked,
    XlmCollateral,
    Guarantor,
}

/// The loan lifecycle, mirroring the `loans.status` CHECK.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    Pending,
    Active,
    Closed,
    Defaulted,
    Declined,
    Cancelled,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    /// One record per loan; key is the loan UUID's 16 bytes.
    Loan(BytesN<16>),
    /// borrower UUID -> their current open loan id. Absent = no open loan.
    BorrowerOpen(BytesN<16>),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Loan {
    pub borrower_id: BytesN<16>,
    pub product: Product,
    pub principal: i128,
    pub rate_bps: u32,
    pub term_months: u32,
    pub policy_version: u64,
    pub status: Status,
    pub principal_outstanding: i128,
    pub disbursed_at: Option<u64>,
    pub closed_at: Option<u64>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoanRecorded {
    #[topic]
    pub loan_id: BytesN<16>,
    pub borrower_id: BytesN<16>,
    pub product: Product,
    pub principal: i128,
    pub rate_bps: u32,
    pub term_months: u32,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatusChanged {
    #[topic]
    pub loan_id: BytesN<16>,
    pub status: Status,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrincipalReduced {
    #[topic]
    pub loan_id: BytesN<16>,
    pub amount: i128,
    pub outstanding: i128,
}

const RECORD_TTL_THRESHOLD: u32 = 518_400; // ~30 days of ledgers
const RECORD_TTL_EXTEND: u32 = 3_110_400; // ~180 days

#[contract]
pub struct LoanRegistry;

#[contractimpl]
impl LoanRegistry {
    /// One-time setup: the admin is the engine's Stellar account.
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    /// Record a new loan in `pending`. Admin-only. Refuses a second open loan
    /// for the same borrower, and pins the price and policy version at birth.
    #[allow(clippy::too_many_arguments)]
    pub fn record_loan(
        env: Env,
        loan_id: BytesN<16>,
        borrower_id: BytesN<16>,
        product: Product,
        principal: i128,
        rate_bps: u32,
        term_months: u32,
        policy_version: u64,
    ) -> Result<(), Error> {
        Self::admin(&env)?.require_auth();
        if principal <= 0 {
            return Err(Error::InvalidAmount);
        }
        if !(1..=2000).contains(&rate_bps) {
            return Err(Error::InvalidRate);
        }
        if !(3..=12).contains(&term_months) {
            return Err(Error::InvalidTerm);
        }

        let loan_key = DataKey::Loan(loan_id.clone());
        if env.storage().persistent().has(&loan_key) {
            return Err(Error::LoanExists);
        }
        let open_key = DataKey::BorrowerOpen(borrower_id.clone());
        if env.storage().persistent().has(&open_key) {
            return Err(Error::BorrowerHasOpenLoan);
        }

        let now = env.ledger().timestamp();
        let loan = Loan {
            borrower_id: borrower_id.clone(),
            product,
            principal,
            rate_bps,
            term_months,
            policy_version,
            status: Status::Pending,
            principal_outstanding: 0,
            disbursed_at: None,
            closed_at: None,
            created_at: now,
            updated_at: now,
        };
        Self::store(&env, &loan_key, &loan);
        env.storage().persistent().set(&open_key, &loan_id);
        env.storage()
            .persistent()
            .extend_ttl(&open_key, RECORD_TTL_THRESHOLD, RECORD_TTL_EXTEND);

        LoanRecorded {
            loan_id,
            borrower_id,
            product,
            principal,
            rate_bps,
            term_months,
        }
        .publish(&env);
        Ok(())
    }

    /// Disburse: `pending` -> `active`. Admin-only. Sets the outstanding
    /// principal to the full amount and records the disbursement time.
    pub fn activate(env: Env, loan_id: BytesN<16>, disbursed_at: u64) -> Result<(), Error> {
        Self::admin(&env)?.require_auth();
        let (key, mut loan) = Self::load(&env, &loan_id)?;
        if loan.status != Status::Pending {
            return Err(Error::NotPending);
        }
        loan.status = Status::Active;
        loan.principal_outstanding = loan.principal;
        loan.disbursed_at = Some(disbursed_at);
        loan.updated_at = env.ledger().timestamp();
        Self::store(&env, &key, &loan);

        StatusChanged { loan_id, status: Status::Active }.publish(&env);
        Ok(())
    }

    /// Apply a principal repayment to an `active` loan. Admin-only.
    pub fn reduce_principal(env: Env, loan_id: BytesN<16>, amount: i128) -> Result<(), Error> {
        Self::admin(&env)?.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let (key, mut loan) = Self::load(&env, &loan_id)?;
        if loan.status != Status::Active {
            return Err(Error::NotActive);
        }
        if amount > loan.principal_outstanding {
            return Err(Error::ReduceExceedsOutstanding);
        }
        loan.principal_outstanding -= amount;
        loan.updated_at = env.ledger().timestamp();
        Self::store(&env, &key, &loan);

        PrincipalReduced {
            loan_id,
            amount,
            outstanding: loan.principal_outstanding,
        }
        .publish(&env);
        Ok(())
    }

    /// Close a fully-repaid `active` loan. Admin-only; refuses to close while
    /// any principal is still outstanding. Frees the borrower's open-loan slot.
    pub fn close(env: Env, loan_id: BytesN<16>, closed_at: u64) -> Result<(), Error> {
        Self::admin(&env)?.require_auth();
        let (key, mut loan) = Self::load(&env, &loan_id)?;
        if loan.status != Status::Active {
            return Err(Error::NotActive);
        }
        if loan.principal_outstanding != 0 {
            return Err(Error::OutstandingRemains);
        }
        Self::finalize(&env, &key, &mut loan, Status::Closed, Some(closed_at));
        StatusChanged { loan_id, status: Status::Closed }.publish(&env);
        Ok(())
    }

    /// Default an `active` loan (recovery runs off-chain / in the vault).
    /// Admin-only. Frees the borrower's open-loan slot.
    pub fn mark_defaulted(env: Env, loan_id: BytesN<16>, closed_at: u64) -> Result<(), Error> {
        Self::admin(&env)?.require_auth();
        let (key, mut loan) = Self::load(&env, &loan_id)?;
        if loan.status != Status::Active {
            return Err(Error::NotActive);
        }
        Self::finalize(&env, &key, &mut loan, Status::Defaulted, Some(closed_at));
        StatusChanged { loan_id, status: Status::Defaulted }.publish(&env);
        Ok(())
    }

    /// Reject a `pending` loan (failed validation / collateral never locked).
    /// Admin-only. Frees the borrower's open-loan slot.
    pub fn decline(env: Env, loan_id: BytesN<16>) -> Result<(), Error> {
        Self::terminate_pending(env, loan_id, Status::Declined)
    }

    /// Borrower withdrew a `pending` request. Admin-only. Frees the slot.
    pub fn cancel(env: Env, loan_id: BytesN<16>) -> Result<(), Error> {
        Self::terminate_pending(env, loan_id, Status::Cancelled)
    }

    /// Read a loan — the engine's reconciliation view.
    pub fn get_loan(env: Env, loan_id: BytesN<16>) -> Option<Loan> {
        env.storage().persistent().get(&DataKey::Loan(loan_id))
    }

    /// The borrower's current open loan id, if any.
    pub fn open_loan_of(env: Env, borrower_id: BytesN<16>) -> Option<BytesN<16>> {
        env.storage()
            .persistent()
            .get(&DataKey::BorrowerOpen(borrower_id))
    }

    // ----- internals -----

    fn terminate_pending(env: Env, loan_id: BytesN<16>, to: Status) -> Result<(), Error> {
        Self::admin(&env)?.require_auth();
        let (key, mut loan) = Self::load(&env, &loan_id)?;
        if loan.status != Status::Pending {
            return Err(Error::NotPending);
        }
        Self::finalize(&env, &key, &mut loan, to, None);
        StatusChanged { loan_id, status: to }.publish(&env);
        Ok(())
    }

    /// Apply a terminal status, stamp times, and release the borrower's
    /// open-loan slot so they may borrow again.
    fn finalize(env: &Env, key: &DataKey, loan: &mut Loan, to: Status, closed_at: Option<u64>) {
        loan.status = to;
        if closed_at.is_some() {
            loan.closed_at = closed_at;
        }
        loan.updated_at = env.ledger().timestamp();
        Self::store(env, key, loan);
        env.storage()
            .persistent()
            .remove(&DataKey::BorrowerOpen(loan.borrower_id.clone()));
    }

    fn load(env: &Env, loan_id: &BytesN<16>) -> Result<(DataKey, Loan), Error> {
        let key = DataKey::Loan(loan_id.clone());
        let loan: Loan = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::LoanNotFound)?;
        Ok((key, loan))
    }

    fn store(env: &Env, key: &DataKey, loan: &Loan) {
        env.storage().persistent().set(key, loan);
        env.storage()
            .persistent()
            .extend_ttl(key, RECORD_TTL_THRESHOLD, RECORD_TTL_EXTEND);
    }

    fn admin(env: &Env) -> Result<Address, Error> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)
    }
}

#[cfg(test)]
mod test;
