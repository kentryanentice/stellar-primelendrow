//! Collateral vault for PrimeLendRow's XLM-collateral loans.
//!
//! Holds native XLM (via its Stellar Asset Contract) locked against a loan,
//! keyed by the loan's UUID bytes. Two properties are what "custody the
//! platform cannot move" means here, and both are enforced on-chain:
//!
//! **1. Every exit is gated on a recorded outcome.** A position starts
//! `Active` and can only leave the vault the way its recorded state allows:
//!
//! ```text
//!   lock ──▶ Active ──mark_repaid──▶ Repaid ────release──▶ depositor
//!              │
//!              └────mark_defaulted─▶ Defaulted ──seize──▶ treasury
//! ```
//!
//!   * `lock`           — anyone may lock *their own* coins against a loan id
//!                        (the borrower signs from their wallet; the engine
//!                        verifies the resulting transaction on Horizon before
//!                        the loan disburses).
//!   * `mark_repaid` /
//!     `mark_defaulted` — ADMIN ONLY. The outcome, recorded on-chain as its
//!                        own transaction, before any money moves.
//!   * `release`        — ADMIN ONLY, and refused unless the loan was recorded
//!                        repaid. Coins go back to the depositor recorded at
//!                        lock time, never to a caller-chosen address.
//!   * `seize`          — ADMIN ONLY, and refused unless a default was
//!                        recorded first. Coins go to the admin-chosen
//!                        treasury.
//!
//! So "collateral on an open loan cannot be released" and "seizure only after
//! a recorded default" are contract behavior, not policy: there is no call
//! sequence that takes coins out of an `Active` position, and no seizure that
//! does not leave a default record on the ledger ahead of it. The admin still
//! decides *what* to record — the contract makes the record unavoidable and
//! public, which is the boundary the SOW claims and no more.
//!
//! **2. Nothing is priced at a number nobody checked.** `lock` and `seize`
//! both take the engine's candidate rate and refuse it unless a public SEP-40
//! feed agrees within a published band (see [`oracle`]). A stale feed or an
//! out-of-band quote fails the transaction; there is no unchecked path.
//!
//! The admin is the engine's Stellar account. A compromised frontend (or a
//! user talking to the contract directly) can put coins in, but can never take
//! any out; a compromised engine key can only move them along the two recorded
//! routes above, at a price the feed vouches for.

#![no_std]

mod oracle;

pub use oracle::{
    Asset, Checked, PriceData, PriceFeedClient, MAX_DEVIATION_BPS, MAX_PRICE_AGE_SECS, QUOTE_SCALE,
};

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
    /// The submitted rates are non-positive, or the peso rate is not
    /// consistent with the two legs it was declared to be crossed from.
    InvalidQuote = 6,
    /// The feed has no price for the configured asset.
    PriceUnavailable = 7,
    /// The feed's newest point is older than [`MAX_PRICE_AGE_SECS`].
    PriceStale = 8,
    /// The quote sits further than [`MAX_DEVIATION_BPS`] from the feed.
    PriceOutOfBand = 9,
    /// The coins offered are worth less than the ratio the vault requires.
    InsufficientCollateral = 10,
    /// Release attempted while the loan is still open.
    LoanStillOpen = 11,
    /// Seizure attempted with no default recorded.
    NoRecordedDefault = 12,
    /// The outcome was already recorded; positions settle once.
    NotActive = 13,
    /// Configuration outside the bounds the contract will accept.
    InvalidConfig = 14,
}

/// Stroops in one XLM — the unit `amount` is counted in.
pub const STROOPS_PER_XLM: i128 = 10_000_000;

/// The lowest collateral ratio the vault will ever accept, whatever the admin
/// sets. The live ratio is a policy parameter (120% this sprint, and expected
/// to rise on any mainnet deployment); *under*-collateralizing is not a policy
/// choice, so 100% is a floor in code.
pub const MIN_COLLATERAL_RATIO_BPS: i128 = 10_000;
/// And a ceiling, so a fat-fingered ratio bricks lending loudly at
/// configuration time rather than quietly refusing every borrower.
pub const MAX_COLLATERAL_RATIO_BPS: i128 = 100_000;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Token,
    Config,
    /// One lock per loan; the key is the loan UUID's 16 bytes.
    Lock(BytesN<16>),
}

/// What the vault does with a loan's collateral is decided by which of these
/// it is in, and only the admin can advance it.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum LoanState {
    /// Coins in, loan open. Nothing leaves the vault from here.
    Active = 0,
    /// Recorded repaid — `release` is now the only exit.
    Repaid = 1,
    /// Recorded defaulted — `seize` is now the only exit.
    Defaulted = 2,
}

/// The feed and the bounds the vault judges quotes against, plus the live
/// collateral ratio. Admin-settable so the vault can follow a feed migration
/// or a recalibrated ratio without a redeploy; the staleness window and the
/// deviation band are NOT here — those are contract constants (see
/// [`MAX_PRICE_AGE_SECS`], [`MAX_DEVIATION_BPS`]), so no admin can widen the
/// check itself.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    /// The SEP-40 price feed contract (Reflector on Stellar).
    pub oracle: Address,
    /// The asset to ask that feed about (Reflector's CEX/DEX feed quotes
    /// `Asset::Other(symbol_short!("XLM"))` against USD).
    pub asset: Asset,
    /// Divides the feed's timestamps into Unix seconds. SEP-40 doesn't fix
    /// the unit and Reflector's own deployments differ, so CHECK THE FEED
    /// rather than assume: the testnet CEX/DEX oracle answers `lastprice`
    /// with 10-digit seconds (divisor 1), while feeds publishing 13-digit
    /// milliseconds need 1000. Getting this wrong doesn't misprice anything
    /// — it makes every price look decades stale and refuses every lock.
    pub feed_time_divisor: u64,
    /// Collateral required as a share of the principal it covers, in basis
    /// points. 12_000 = the 120% this sprint locks at.
    pub collateral_ratio_bps: i128,
}

/// The engine's candidate rate, as submitted with a lock or a seizure.
///
/// Two legs and their product, because no public feed quotes XLM in pesos with
/// real depth: the crypto venues quote XLM/USD, the fiat feeds quote USD/PHP,
/// and `php_per_xlm_centavos` is what the engine derived and recorded with the
/// loan. Only the USD leg can be checked against a Stellar feed; the peso rate
/// is checked for *consistency* with the legs beside it, and the fiat leg is
/// recorded but unverifiable on-chain.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Quote {
    /// Whole centavos one XLM is worth — the rate the collateral is valued at.
    pub php_per_xlm_centavos: i128,
    /// USD per XLM, scaled by [`QUOTE_SCALE`] — the leg the feed checks.
    pub usd_per_xlm_e8: i128,
    /// Whole centavos one USD is worth, the leg the USD quote was crossed
    /// through. Recorded for the proof page; no Stellar feed can check it.
    pub php_per_usd_centavos: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lock {
    pub depositor: Address,
    /// Stroops held.
    pub amount: i128,
    pub locked_at: u64,
    pub state: LoanState,
    /// When the state last moved — the recorded-default timestamp once
    /// defaulted, which is the thing a seizure has to point back at.
    pub state_at: u64,
    /// Centavos of principal this collateral covers.
    pub principal_centavos: i128,
    /// The rate accepted at lock time, pinned: a later price move never
    /// rewrites what this borrower was asked to lock.
    pub quote: Quote,
    /// What the feed said when that rate was accepted.
    pub feed: Checked,
    /// The ratio enforced at lock time, pinned for the same reason.
    pub ratio_bps: i128,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Locked {
    #[topic]
    pub loan_id: BytesN<16>,
    pub depositor: Address,
    pub amount: i128,
    pub principal_centavos: i128,
    pub php_per_xlm_centavos: i128,
    pub feed_usd_per_xlm_e8: i128,
    pub feed_at: u64,
}

/// The outcome record every exit from the vault has to point back at.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Recorded {
    #[topic]
    pub loan_id: BytesN<16>,
    pub state: LoanState,
    pub at: u64,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Released {
    #[topic]
    pub loan_id: BytesN<16>,
    pub to: Address,
    pub amount: i128,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Seized {
    #[topic]
    pub loan_id: BytesN<16>,
    pub to: Address,
    pub amount: i128,
    /// What the seized coins were worth at the checked seizure price — the
    /// number that decides how much debt they cover before any guarantor is
    /// charged.
    pub value_centavos: i128,
    pub php_per_xlm_centavos: i128,
    pub feed_usd_per_xlm_e8: i128,
    pub feed_at: u64,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Configured {
    pub oracle: Address,
    pub collateral_ratio_bps: i128,
}

/// Locks live in persistent storage and are bumped on every touch so an
/// active loan's collateral cannot silently expire out of the ledger.
const LOCK_TTL_THRESHOLD: u32 = 518_400; // ~30 days of ledgers
const LOCK_TTL_EXTEND: u32 = 3_110_400; // ~180 days

#[contract]
pub struct CollateralVault;

#[contractimpl]
impl CollateralVault {
    /// One-time setup: the admin (the engine's account), the token the vault
    /// accepts (native XLM's Stellar Asset Contract), and the price feed and
    /// ratio it enforces.
    pub fn initialize(
        env: Env,
        admin: Address,
        token: Address,
        oracle: Address,
        asset: Asset,
        feed_time_divisor: u64,
        collateral_ratio_bps: i128,
    ) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        let config = validated_config(oracle, asset, feed_time_divisor, collateral_ratio_bps)?;
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Token, &token);
        env.storage().instance().set(&DataKey::Config, &config);

        Configured {
            oracle: config.oracle,
            collateral_ratio_bps: config.collateral_ratio_bps,
        }
        .publish(&env);
        Ok(())
    }

    /// Point the vault at a different feed, or move the ratio. Admin-only, and
    /// bounded: the staleness window and deviation band are constants, and the
    /// ratio cannot go below [`MIN_COLLATERAL_RATIO_BPS`]. Positions already
    /// locked keep the rate and ratio they were struck at.
    pub fn configure(
        env: Env,
        oracle: Address,
        asset: Asset,
        feed_time_divisor: u64,
        collateral_ratio_bps: i128,
    ) -> Result<(), Error> {
        Self::admin(&env)?.require_auth();
        let config = validated_config(oracle, asset, feed_time_divisor, collateral_ratio_bps)?;
        env.storage().instance().set(&DataKey::Config, &config);

        Configured {
            oracle: config.oracle,
            collateral_ratio_bps: config.collateral_ratio_bps,
        }
        .publish(&env);
        Ok(())
    }

    /// Borrower locks `amount` stroops against `loan_id`, covering
    /// `principal_centavos` of loan at the submitted rate.
    ///
    /// Requires the depositor's own signature — the transfer is from their
    /// account into this contract, so nobody can lock someone else's funds.
    /// The rate is checked against the feed and the coins are checked against
    /// the ratio BEFORE anything moves: a refused lock costs the borrower a
    /// failed transaction, never a stranded balance.
    pub fn lock(
        env: Env,
        depositor: Address,
        loan_id: BytesN<16>,
        amount: i128,
        principal_centavos: i128,
        quote: Quote,
    ) -> Result<(), Error> {
        depositor.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        if principal_centavos <= 0 {
            return Err(Error::InvalidQuote);
        }
        let key = DataKey::Lock(loan_id.clone());
        if env.storage().persistent().has(&key) {
            // One lock per loan: topping up would complicate the engine's
            // verification (one tx hash <-> one position), so it is refused.
            return Err(Error::LockExists);
        }

        let config = Self::config(&env)?;
        let feed = check_quote(&env, &config, &quote)?;

        // The ratio, at the rate the feed just vouched for: what the coins are
        // worth must cover `ratio_bps` of the principal they stand behind.
        let value_centavos = amount * quote.php_per_xlm_centavos / STROOPS_PER_XLM;
        if value_centavos * 10_000 < principal_centavos * config.collateral_ratio_bps {
            return Err(Error::InsufficientCollateral);
        }

        let token_id = Self::token(&env)?;
        token::TokenClient::new(&env, &token_id).transfer(
            &depositor,
            &env.current_contract_address(),
            &amount,
        );

        let now = env.ledger().timestamp();
        let lock = Lock {
            depositor: depositor.clone(),
            amount,
            locked_at: now,
            state: LoanState::Active,
            state_at: now,
            principal_centavos,
            quote: quote.clone(),
            feed: feed.clone(),
            ratio_bps: config.collateral_ratio_bps,
        };
        env.storage().persistent().set(&key, &lock);
        env.storage()
            .persistent()
            .extend_ttl(&key, LOCK_TTL_THRESHOLD, LOCK_TTL_EXTEND);

        Locked {
            loan_id,
            depositor,
            amount,
            principal_centavos,
            php_per_xlm_centavos: quote.php_per_xlm_centavos,
            feed_usd_per_xlm_e8: feed.feed_usd_per_xlm_e8,
            feed_at: feed.feed_at,
        }
        .publish(&env);
        Ok(())
    }

    /// Record the loan as fully repaid. Admin-only, and the only thing that
    /// opens `release`.
    pub fn mark_repaid(env: Env, loan_id: BytesN<16>) -> Result<(), Error> {
        Self::record(env, loan_id, LoanState::Repaid)
    }

    /// Record the loan as defaulted. Admin-only, and the only thing that opens
    /// `seize` — so every seizure has a public default record ahead of it.
    pub fn mark_defaulted(env: Env, loan_id: BytesN<16>) -> Result<(), Error> {
        Self::record(env, loan_id, LoanState::Defaulted)
    }

    /// Loan repaid: coins go home. Admin-only, refused unless the repayment
    /// was recorded first, and the destination is the depositor recorded at
    /// lock time — the admin cannot redirect it, so even a compromised engine
    /// key can't quietly route releases elsewhere.
    pub fn release(env: Env, loan_id: BytesN<16>) -> Result<(), Error> {
        Self::admin(&env)?.require_auth();
        let key = DataKey::Lock(loan_id.clone());
        let lock = Self::lock_at(&env, &key)?;
        if lock.state != LoanState::Repaid {
            // Includes the defaulted case: coins on a defaulted loan leave by
            // seizure or not at all.
            return Err(Error::LoanStillOpen);
        }

        let token_id = Self::token(&env)?;
        token::TokenClient::new(&env, &token_id).transfer(
            &env.current_contract_address(),
            &lock.depositor,
            &lock.amount,
        );
        env.storage().persistent().remove(&key);

        Released { loan_id, to: lock.depositor, amount: lock.amount }.publish(&env);
        Ok(())
    }

    /// Default/liquidation: coins go to the treasury `to`, valued at a
    /// freshly checked seizure price. Admin-only, and refused unless a default
    /// was recorded first.
    pub fn seize(env: Env, loan_id: BytesN<16>, to: Address, quote: Quote) -> Result<(), Error> {
        Self::admin(&env)?.require_auth();
        let key = DataKey::Lock(loan_id.clone());
        let lock = Self::lock_at(&env, &key)?;
        if lock.state != LoanState::Defaulted {
            return Err(Error::NoRecordedDefault);
        }

        let config = Self::config(&env)?;
        let feed = check_quote(&env, &config, &quote)?;
        // The seizure price is what decides how much debt these coins cover
        // before any guarantor is charged, so it is recorded, not inferred.
        let value_centavos = lock.amount * quote.php_per_xlm_centavos / STROOPS_PER_XLM;

        let token_id = Self::token(&env)?;
        token::TokenClient::new(&env, &token_id).transfer(
            &env.current_contract_address(),
            &to,
            &lock.amount,
        );
        env.storage().persistent().remove(&key);

        Seized {
            loan_id,
            to,
            amount: lock.amount,
            value_centavos,
            php_per_xlm_centavos: quote.php_per_xlm_centavos,
            feed_usd_per_xlm_e8: feed.feed_usd_per_xlm_e8,
            feed_at: feed.feed_at,
        }
        .publish(&env);
        Ok(())
    }

    /// Read a position — the engine's reconciliation view, and what the proof
    /// page renders: the coins, the outcome recorded against them, and the
    /// checked price they were struck at.
    pub fn get_lock(env: Env, loan_id: BytesN<16>) -> Option<Lock> {
        env.storage().persistent().get(&DataKey::Lock(loan_id))
    }

    /// The feed, ratio and admin this vault is running with — published so a
    /// reviewer can check the authorization model against the deployment
    /// rather than against a README.
    pub fn get_config(env: Env) -> Result<Config, Error> {
        Self::config(&env)
    }

    pub fn get_admin(env: Env) -> Result<Address, Error> {
        Self::admin(&env)
    }

    fn record(env: Env, loan_id: BytesN<16>, state: LoanState) -> Result<(), Error> {
        Self::admin(&env)?.require_auth();
        let key = DataKey::Lock(loan_id.clone());
        let mut lock = Self::lock_at(&env, &key)?;
        if lock.state != LoanState::Active {
            // An outcome is recorded once. Re-recording would let a repayment
            // be turned into a default after the fact.
            return Err(Error::NotActive);
        }
        lock.state = state;
        lock.state_at = env.ledger().timestamp();
        env.storage().persistent().set(&key, &lock);
        env.storage()
            .persistent()
            .extend_ttl(&key, LOCK_TTL_THRESHOLD, LOCK_TTL_EXTEND);

        Recorded { loan_id, state, at: lock.state_at }.publish(&env);
        Ok(())
    }

    fn lock_at(env: &Env, key: &DataKey) -> Result<Lock, Error> {
        env.storage().persistent().get(key).ok_or(Error::LockNotFound)
    }

    fn admin(env: &Env) -> Result<Address, Error> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)
    }

    fn token(env: &Env) -> Result<Address, Error> {
        env.storage()
            .instance()
            .get(&DataKey::Token)
            .ok_or(Error::NotInitialized)
    }

    fn config(env: &Env) -> Result<Config, Error> {
        env.storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(Error::NotInitialized)
    }
}

fn validated_config(
    oracle: Address,
    asset: Asset,
    feed_time_divisor: u64,
    collateral_ratio_bps: i128,
) -> Result<Config, Error> {
    if feed_time_divisor == 0
        || !(MIN_COLLATERAL_RATIO_BPS..=MAX_COLLATERAL_RATIO_BPS).contains(&collateral_ratio_bps)
    {
        return Err(Error::InvalidConfig);
    }
    Ok(Config { oracle, asset, feed_time_divisor, collateral_ratio_bps })
}

/// The full price gate both money-moving entry points run: the USD leg is
/// judged against the feed, and the peso rate is judged against the two legs
/// it claims to come from. Passing the first but not the second would mean a
/// checkable quote wrapped around an arbitrary peso number — which is exactly
/// the discretion the check exists to remove.
fn check_quote(env: &Env, config: &Config, quote: &Quote) -> Result<Checked, Error> {
    if quote.php_per_xlm_centavos <= 0 || quote.php_per_usd_centavos <= 0 {
        return Err(Error::InvalidQuote);
    }
    let checked = oracle::check(env, config, quote.usd_per_xlm_e8)?;

    let derived_centavos = quote.usd_per_xlm_e8 * quote.php_per_usd_centavos / QUOTE_SCALE;
    if derived_centavos <= 0
        || oracle::deviates(quote.php_per_xlm_centavos, derived_centavos, MAX_DEVIATION_BPS)
    {
        return Err(Error::InvalidQuote);
    }
    Ok(checked)
}

#[cfg(test)]
mod test;
