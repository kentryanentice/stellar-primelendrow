//! Collateral vault for PrimeLendRow's XLM-collateral loans.
//!
//! Holds native XLM (via its Stellar Asset Contract) locked against a loan,
//! keyed by the loan's UUID bytes. The security model is deliberately
//! asymmetric — it is what "all releases are backend gated" means on-chain:
//!
//!   * `lock`    — anyone may lock *their own* coins against a loan id
//!                 (the borrower signs this from their wallet; the engine
//!                 verifies the resulting transaction on Horizon before the
//!                 loan disburses).
//!   * `release` — ADMIN ONLY. Coins go back to the depositor recorded at
//!                 lock time, never to a caller-chosen address.
//!   * `seize`   — ADMIN ONLY. Default/liquidation path; coins go to the
//!                 admin-chosen treasury address.
//!
//! The admin is the engine's Stellar account. A compromised frontend (or a
//! user talking to the contract directly) can put coins in, but can never
//! take any out.

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, token, Address, BytesN,
    Env,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InvalidAmount = 3,
    LockExists = 4,
    LockNotFound = 5,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Token,
    /// One lock per loan; the key is the loan UUID's 16 bytes.
    Lock(BytesN<16>),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lock {
    pub depositor: Address,
    pub amount: i128,
    pub locked_at: u64,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Locked {
    #[topic]
    pub loan_id: BytesN<16>,
    pub depositor: Address,
    pub amount: i128,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Released {
    #[topic]
    pub loan_id: BytesN<16>,
    pub amount: i128,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Seized {
    #[topic]
    pub loan_id: BytesN<16>,
    pub to: Address,
    pub amount: i128,
}

/// Locks live in persistent storage and are bumped on every touch so an
/// active loan's collateral cannot silently expire out of the ledger.
const LOCK_TTL_THRESHOLD: u32 = 518_400; // ~30 days of ledgers
const LOCK_TTL_EXTEND: u32 = 3_110_400; // ~180 days

#[contract]
pub struct CollateralVault;

#[contractimpl]
impl CollateralVault {
    /// One-time setup: the admin (the engine's account) and the token the
    /// vault accepts (native XLM's Stellar Asset Contract address).
    pub fn initialize(env: Env, admin: Address, token: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Token, &token);
        Ok(())
    }

    /// Borrower locks `amount` stroops against `loan_id`. Requires the
    /// depositor's own signature — the transfer is from their account into
    /// this contract, so nobody can lock someone else's funds.
    pub fn lock(env: Env, depositor: Address, loan_id: BytesN<16>, amount: i128) -> Result<(), Error> {
        depositor.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let key = DataKey::Lock(loan_id.clone());
        if env.storage().persistent().has(&key) {
            // One lock per loan: topping up would complicate the engine's
            // verification (one tx hash <-> one position), so it is refused.
            return Err(Error::LockExists);
        }

        let token_id: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .ok_or(Error::NotInitialized)?;
        token::TokenClient::new(&env, &token_id).transfer(
            &depositor,
            &env.current_contract_address(),
            &amount,
        );

        let lock = Lock {
            depositor: depositor.clone(),
            amount,
            locked_at: env.ledger().timestamp(),
        };
        env.storage().persistent().set(&key, &lock);
        env.storage()
            .persistent()
            .extend_ttl(&key, LOCK_TTL_THRESHOLD, LOCK_TTL_EXTEND);

        Locked { loan_id, depositor, amount }.publish(&env);
        Ok(())
    }

    /// Loan repaid: coins go home. Admin-only, and the destination is the
    /// depositor recorded at lock time — the admin cannot redirect it, so
    /// even a compromised engine key can't quietly route releases elsewhere.
    pub fn release(env: Env, loan_id: BytesN<16>) -> Result<(), Error> {
        Self::admin(&env)?.require_auth();
        let key = DataKey::Lock(loan_id.clone());
        let lock: Lock = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::LockNotFound)?;

        let token_id: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .ok_or(Error::NotInitialized)?;
        token::TokenClient::new(&env, &token_id).transfer(
            &env.current_contract_address(),
            &lock.depositor,
            &lock.amount,
        );
        env.storage().persistent().remove(&key);

        Released { loan_id, amount: lock.amount }.publish(&env);
        Ok(())
    }

    /// Default/liquidation: coins go to the treasury `to`. Admin-only.
    pub fn seize(env: Env, loan_id: BytesN<16>, to: Address) -> Result<(), Error> {
        Self::admin(&env)?.require_auth();
        let key = DataKey::Lock(loan_id.clone());
        let lock: Lock = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::LockNotFound)?;

        let token_id: Address = env
            .storage()
            .instance()
            .get(&DataKey::Token)
            .ok_or(Error::NotInitialized)?;
        token::TokenClient::new(&env, &token_id).transfer(
            &env.current_contract_address(),
            &to,
            &lock.amount,
        );
        env.storage().persistent().remove(&key);

        Seized { loan_id, to, amount: lock.amount }.publish(&env);
        Ok(())
    }

    /// Read a position — the engine's reconciliation view.
    pub fn get_lock(env: Env, loan_id: BytesN<16>) -> Option<Lock> {
        env.storage().persistent().get(&DataKey::Lock(loan_id))
    }

    fn admin(env: &Env) -> Result<Address, Error> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    fn setup(env: &Env) -> (CollateralVaultClient<'_>, Address, Address, token::StellarAssetClient<'_>, token::TokenClient<'_>) {
        env.mock_all_auths();
        let admin = Address::generate(env);
        let depositor = Address::generate(env);

        let sac = env.register_stellar_asset_contract_v2(admin.clone());
        let asset = token::StellarAssetClient::new(env, &sac.address());
        let token_client = token::TokenClient::new(env, &sac.address());

        let contract_id = env.register(CollateralVault, ());
        let vault = CollateralVaultClient::new(env, &contract_id);
        vault.initialize(&admin, &sac.address());

        asset.mint(&depositor, &1_000_0000000);
        (vault, admin, depositor, asset, token_client)
    }

    fn loan_id(env: &Env) -> BytesN<16> {
        BytesN::from_array(env, &[7u8; 16])
    }

    #[test]
    fn lock_then_release_returns_funds_to_depositor() {
        let env = Env::default();
        let (vault, _admin, depositor, _asset, token_client) = setup(&env);
        let id = loan_id(&env);

        vault.lock(&depositor, &id, &250_0000000);
        assert_eq!(token_client.balance(&depositor), 750_0000000);
        assert_eq!(vault.get_lock(&id).unwrap().amount, 250_0000000);

        vault.release(&id);
        assert_eq!(token_client.balance(&depositor), 1_000_0000000);
        assert!(vault.get_lock(&id).is_none());
    }

    #[test]
    fn seize_sends_funds_to_treasury() {
        let env = Env::default();
        let (vault, _admin, depositor, _asset, token_client) = setup(&env);
        let treasury = Address::generate(&env);
        let id = loan_id(&env);

        vault.lock(&depositor, &id, &100_0000000);
        vault.seize(&id, &treasury);

        assert_eq!(token_client.balance(&treasury), 100_0000000);
        assert!(vault.get_lock(&id).is_none());
    }

    #[test]
    fn duplicate_lock_for_same_loan_is_refused() {
        let env = Env::default();
        let (vault, _admin, depositor, _asset, _token_client) = setup(&env);
        let id = loan_id(&env);

        vault.lock(&depositor, &id, &10_0000000);
        assert_eq!(
            vault.try_lock(&depositor, &id, &10_0000000),
            Err(Ok(Error::LockExists))
        );
    }

    #[test]
    fn release_requires_admin_auth() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let depositor = Address::generate(&env);
        let sac = env.register_stellar_asset_contract_v2(admin.clone());
        token::StellarAssetClient::new(&env, &sac.address()).mint(&depositor, &50_0000000);

        let contract_id = env.register(CollateralVault, ());
        let vault = CollateralVaultClient::new(&env, &contract_id);
        vault.initialize(&admin, &sac.address());
        vault.lock(&depositor, &loan_id(&env), &50_0000000);

        // From here, only allow explicitly-listed auths: none. The release
        // must then fail auth, proving a non-admin (or unsigned) call cannot
        // move funds out.
        env.set_auths(&[]);
        assert!(vault.try_release(&loan_id(&env)).is_err());
    }
}
