//! On-chain payment registry for PrimeLendRow.
//!
//! A tamper-evident, engine-authored mirror of the `loan_payments` table
//! (migration 023): one record per captured PayPal repayment, carrying the
//! engine's interest/principal split. It does **not** move money — the real
//! peso capture happens at PayPal and the Postgres ledger is the source of
//! truth. This contract puts the *history* on Stellar, where it cannot be
//! rewritten.
//!
//! Two properties are enforced, matching the engine's table:
//!   * **append-only** — there is no update or delete function; a record, once
//!     written, is permanent;
//!   * **deduplicated by rail reference** — each record is keyed by its PayPal
//!     capture id (`rail_ref`), so a resent capture callback cannot double-log
//!     a payment. Mirrors the `UNIQUE (rail_ref)` index.
//!
//! Same trust model as the collateral vault: **only the admin (the engine's
//! Stellar account) can write.** Reads are open. Amounts are whole centavos.

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, Address, BytesN, Env,
    String,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InvalidAmount = 3,   // amount_received not > 0, or a split part < 0
    PaymentExists = 4,   // this rail_ref was already recorded (dedup)
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    /// One record per PayPal capture; the key is the capture id (`rail_ref`).
    Payment(String),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Payment {
    pub loan_id: BytesN<16>,
    pub user_id: BytesN<16>,
    pub amount_received: i128,
    pub interest_paid: i128,
    pub principal_paid: i128,
    /// Overpayment the engine turned into a fresh `available` deposit lot.
    pub excess: i128,
    pub paid_at: u64,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaymentRecorded {
    #[topic]
    pub loan_id: BytesN<16>,
    pub user_id: BytesN<16>,
    pub rail_ref: String,
    pub amount_received: i128,
}

const RECORD_TTL_THRESHOLD: u32 = 518_400; // ~30 days of ledgers
const RECORD_TTL_EXTEND: u32 = 3_110_400; // ~180 days

#[contract]
pub struct PaymentRegistry;

#[contractimpl]
impl PaymentRegistry {
    /// One-time setup: the admin is the engine's Stellar account.
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    /// Record a captured repayment. Admin-only, append-only, and deduplicated
    /// by `rail_ref` — a second call with the same capture id is refused.
    #[allow(clippy::too_many_arguments)]
    pub fn record_payment(
        env: Env,
        rail_ref: String,
        loan_id: BytesN<16>,
        user_id: BytesN<16>,
        amount_received: i128,
        interest_paid: i128,
        principal_paid: i128,
        excess: i128,
        paid_at: u64,
    ) -> Result<(), Error> {
        Self::admin(&env)?.require_auth();
        if amount_received <= 0
            || interest_paid < 0
            || principal_paid < 0
            || excess < 0
        {
            return Err(Error::InvalidAmount);
        }

        let key = DataKey::Payment(rail_ref.clone());
        if env.storage().persistent().has(&key) {
            return Err(Error::PaymentExists);
        }

        let payment = Payment {
            loan_id: loan_id.clone(),
            user_id: user_id.clone(),
            amount_received,
            interest_paid,
            principal_paid,
            excess,
            paid_at,
        };
        env.storage().persistent().set(&key, &payment);
        env.storage()
            .persistent()
            .extend_ttl(&key, RECORD_TTL_THRESHOLD, RECORD_TTL_EXTEND);

        PaymentRecorded {
            loan_id,
            user_id,
            rail_ref,
            amount_received,
        }
        .publish(&env);
        Ok(())
    }

    /// Read a payment by its capture id — the engine's reconciliation view.
    pub fn get_payment(env: Env, rail_ref: String) -> Option<Payment> {
        env.storage().persistent().get(&DataKey::Payment(rail_ref))
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
