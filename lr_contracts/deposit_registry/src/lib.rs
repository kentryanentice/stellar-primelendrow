//! On-chain deposit registry for PrimeLendRow.
//!
//! The real pesos move through PayPal and the source of truth for the books
//! is the engine's Postgres ledger. This contract does **not** custody money —
//! it is a tamper-evident, engine-authored mirror of the `deposits` table
//! (migration 019): every deposit lot, its badge, and the loan it is working
//! for, recorded on Stellar where nobody — not even a compromised app
//! credential — can rewrite history.
//!
//! Same trust model as the collateral vault: **only the admin (the engine's
//! Stellar account) can write.** Reads are open to anyone. A deposit lot
//! carries exactly one badge, and the (badge ↔ backing_loan) pair is checked
//! on every write so an illegal state can't be recorded even by accident:
//!
//!   * `available`  — free, withdrawable, backing nothing (no loan)
//!   * `lent`       — funding someone's active loan            (has a loan)
//!   * `collateral` — backing the owner's own deposit-backed loan (has a loan)
//!   * `pledged`    — backing someone else's guarantor loan     (has a loan)
//!   * `withdrawn`  — terminal; the lot has left the pool
//!
//! Amounts are whole centavos, mirroring the engine (`BIGINT`). Lots are keyed
//! by the deposit's UUID (its 16 bytes).

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
    DepositExists = 4,
    DepositNotFound = 5,
    /// A working badge without a backing loan, or a free/terminal badge that
    /// names one — the (badge ↔ backing_loan) invariant was violated.
    IllegalBadgeState = 6,
    /// Tried to move or withdraw a lot that is no longer `available`.
    NotAvailable = 7,
    /// Tried to mutate a lot that has already been withdrawn.
    AlreadyWithdrawn = 8,
}

/// The one badge a lot may carry — mirrors the `deposits.badge` CHECK.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Badge {
    Available,
    Lent,
    Collateral,
    Pledged,
    Withdrawn,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    /// One record per deposit; the key is the deposit UUID's 16 bytes.
    Deposit(BytesN<16>),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Deposit {
    pub user_id: BytesN<16>,
    pub amount: i128,
    pub badge: Badge,
    /// Set iff the lot is working for a loan (`lent`/`collateral`/`pledged`).
    pub backing_loan: Option<BytesN<16>>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DepositRecorded {
    #[topic]
    pub deposit_id: BytesN<16>,
    pub user_id: BytesN<16>,
    pub amount: i128,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BadgeChanged {
    #[topic]
    pub deposit_id: BytesN<16>,
    pub badge: Badge,
    pub backing_loan: Option<BytesN<16>>,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Withdrawn {
    #[topic]
    pub deposit_id: BytesN<16>,
    pub amount: i128,
}

/// Records live in persistent storage and are bumped on every touch so an
/// active pool's history cannot silently expire out of the ledger.
const RECORD_TTL_THRESHOLD: u32 = 518_400; // ~30 days of ledgers
const RECORD_TTL_EXTEND: u32 = 3_110_400; // ~180 days

#[contract]
pub struct DepositRegistry;

#[contractimpl]
impl DepositRegistry {
    /// One-time setup: the admin is the engine's Stellar account, the only
    /// caller allowed to write records.
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    /// Record a new deposit lot. Admin-only. A fresh lot always starts
    /// `available` and backing nothing.
    pub fn record_deposit(
        env: Env,
        deposit_id: BytesN<16>,
        user_id: BytesN<16>,
        amount: i128,
    ) -> Result<(), Error> {
        Self::admin(&env)?.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let key = DataKey::Deposit(deposit_id.clone());
        if env.storage().persistent().has(&key) {
            return Err(Error::DepositExists);
        }

        let now = env.ledger().timestamp();
        let deposit = Deposit {
            user_id: user_id.clone(),
            amount,
            badge: Badge::Available,
            backing_loan: None,
            created_at: now,
            updated_at: now,
        };
        Self::store(&env, &key, &deposit);

        DepositRecorded { deposit_id, user_id, amount }.publish(&env);
        Ok(())
    }

    /// Move a lot's badge — e.g. `available` → `collateral` when it starts
    /// backing a loan, or back to `available` when the loan closes. Admin-only.
    /// The (badge ↔ backing_loan) invariant is enforced here.
    pub fn set_badge(
        env: Env,
        deposit_id: BytesN<16>,
        badge: Badge,
        backing_loan: Option<BytesN<16>>,
    ) -> Result<(), Error> {
        Self::admin(&env)?.require_auth();
        // `withdrawn` is terminal and reached only through `withdraw`.
        if badge == Badge::Withdrawn {
            return Err(Error::IllegalBadgeState);
        }
        Self::check_pairing(badge, &backing_loan)?;

        let key = DataKey::Deposit(deposit_id.clone());
        let mut deposit: Deposit = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::DepositNotFound)?;
        if deposit.badge == Badge::Withdrawn {
            return Err(Error::AlreadyWithdrawn);
        }

        deposit.badge = badge;
        deposit.backing_loan = backing_loan.clone();
        deposit.updated_at = env.ledger().timestamp();
        Self::store(&env, &key, &deposit);

        BadgeChanged { deposit_id, badge, backing_loan }.publish(&env);
        Ok(())
    }

    /// Withdraw a lot out of the pool. Admin-only, and only an `available` lot
    /// may leave — a lot working for a loan cannot be withdrawn.
    pub fn withdraw(env: Env, deposit_id: BytesN<16>) -> Result<(), Error> {
        Self::admin(&env)?.require_auth();
        let key = DataKey::Deposit(deposit_id.clone());
        let mut deposit: Deposit = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::DepositNotFound)?;
        if deposit.badge == Badge::Withdrawn {
            return Err(Error::AlreadyWithdrawn);
        }
        if deposit.badge != Badge::Available {
            return Err(Error::NotAvailable);
        }

        deposit.badge = Badge::Withdrawn;
        deposit.backing_loan = None;
        deposit.updated_at = env.ledger().timestamp();
        Self::store(&env, &key, &deposit);

        Withdrawn { deposit_id, amount: deposit.amount }.publish(&env);
        Ok(())
    }

    /// Read a deposit lot — the engine's reconciliation view.
    pub fn get_deposit(env: Env, deposit_id: BytesN<16>) -> Option<Deposit> {
        env.storage()
            .persistent()
            .get(&DataKey::Deposit(deposit_id))
    }

    fn store(env: &Env, key: &DataKey, deposit: &Deposit) {
        env.storage().persistent().set(key, deposit);
        env.storage()
            .persistent()
            .extend_ttl(key, RECORD_TTL_THRESHOLD, RECORD_TTL_EXTEND);
    }

    /// A working badge MUST name the loan it backs; a free badge must NOT.
    fn check_pairing(badge: Badge, backing_loan: &Option<BytesN<16>>) -> Result<(), Error> {
        let working = matches!(badge, Badge::Lent | Badge::Collateral | Badge::Pledged);
        if working != backing_loan.is_some() {
            return Err(Error::IllegalBadgeState);
        }
        Ok(())
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
