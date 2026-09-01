#![cfg(test)]

//! The two safety properties the SOW puts its name to (§3.10) are the first
//! four tests here: collateral on an open loan cannot be released, seizure
//! cannot happen without a recorded default, a stale feed refuses to act, and
//! an out-of-band quote refuses to act. The rest hold the edges around them.

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{contract, contractimpl, Symbol};

/// A ledger clock the price ages are measured against — the default is 0,
/// which would make every feed look fresh.
const NOW: u64 = 1_700_000_000;

/// Reflector publishes 14 decimals and millisecond timestamps; the mock does
/// the same, so the rescaling and the divisor are exercised, not bypassed.
const FEED_DECIMALS: u32 = 14;
const FEED_TIME_DIVISOR: u64 = 1000;

/// $0.40 per XLM, as the feed reports it (14 decimals) and as the engine
/// submits it (scaled 1e8).
const FEED_PRICE: i128 = 40_000_000_000_000;
const USD_PER_XLM_E8: i128 = 40_000_000;
/// ₱50.00 per USD, so one XLM is worth ₱20.00 — 2000 centavos.
const PHP_PER_USD: i128 = 5_000;
const PHP_PER_XLM: i128 = 2_000;

/// A ₱5,000 loan at 120% needs ₱6,000 of XLM, which at ₱20.00 is 300 XLM.
const PRINCIPAL: i128 = 500_000;
const REQUIRED_STROOPS: i128 = 300 * STROOPS_PER_XLM;
const RATIO_BPS: i128 = 12_000;

// ---- a SEP-40 feed under test control --------------------------------------

#[contracttype]
enum FeedKey {
    Point,
    Decimals,
}

#[contract]
pub struct MockFeed;

#[contractimpl]
impl MockFeed {
    pub fn set(env: Env, price: i128, timestamp: u64, decimals: u32) {
        env.storage()
            .instance()
            .set(&FeedKey::Point, &PriceData { price, timestamp });
        env.storage().instance().set(&FeedKey::Decimals, &decimals);
    }

    /// A feed that has nothing to say about the asset — the "no usable feed"
    /// case the vault has to fail closed on.
    pub fn silence(env: Env) {
        env.storage().instance().remove(&FeedKey::Point);
    }

    pub fn decimals(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&FeedKey::Decimals)
            .unwrap_or(FEED_DECIMALS)
    }

    pub fn lastprice(env: Env, _asset: Asset) -> Option<PriceData> {
        env.storage().instance().get(&FeedKey::Point)
    }
}

// ---- fixture ---------------------------------------------------------------

struct World<'a> {
    vault: CollateralVaultClient<'a>,
    feed: MockFeedClient<'a>,
    token: token::TokenClient<'a>,
    admin: Address,
    depositor: Address,
}

const MINTED: i128 = 1_000 * STROOPS_PER_XLM;

fn setup(env: &Env) -> World<'_> {
    env.mock_all_auths();
    env.ledger().set_timestamp(NOW);

    let admin = Address::generate(env);
    let depositor = Address::generate(env);

    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    token::StellarAssetClient::new(env, &sac.address()).mint(&depositor, &MINTED);

    let feed = MockFeedClient::new(env, &env.register(MockFeed, ()));
    feed.set(&FEED_PRICE, &(NOW * 1000), &FEED_DECIMALS);

    let vault = CollateralVaultClient::new(env, &env.register(CollateralVault, ()));
    vault.initialize(
        &admin,
        &sac.address(),
        &feed.address,
        &Asset::Other(Symbol::new(env, "XLM")),
        &FEED_TIME_DIVISOR,
        &RATIO_BPS,
    );

    World {
        vault,
        feed,
        token: token::TokenClient::new(env, &sac.address()),
        admin,
        depositor,
    }
}

fn loan_id(env: &Env) -> BytesN<16> {
    BytesN::from_array(env, &[7u8; 16])
}

/// The engine's candidate rate, honest by default.
fn quote() -> Quote {
    Quote {
        php_per_xlm_centavos: PHP_PER_XLM,
        usd_per_xlm_e8: USD_PER_XLM_E8,
        php_per_usd_centavos: PHP_PER_USD,
    }
}

fn lock(w: &World, id: &BytesN<16>) {
    w.vault
        .lock(&w.depositor, id, &REQUIRED_STROOPS, &PRINCIPAL, &quote());
}

// ---- the properties the SOW commits to -------------------------------------

#[test]
fn collateral_on_an_open_loan_cannot_be_released() {
    let env = Env::default();
    let w = setup(&env);
    let id = loan_id(&env);
    lock(&w, &id);

    // Admin-signed, correctly formed, and still refused: the loan is Active
    // and no outcome has been recorded against it.
    assert_eq!(w.vault.try_release(&id), Err(Ok(Error::LoanStillOpen)));
    assert_eq!(w.token.balance(&w.depositor), MINTED - REQUIRED_STROOPS);
    assert_eq!(w.vault.get_lock(&id).unwrap().state, LoanState::Active);
}

#[test]
fn seizure_is_refused_without_a_recorded_default() {
    let env = Env::default();
    let w = setup(&env);
    let treasury = Address::generate(&env);
    let id = loan_id(&env);
    lock(&w, &id);

    assert_eq!(
        w.vault.try_seize(&id, &treasury, &quote()),
        Err(Ok(Error::NoRecordedDefault))
    );
    // Nor by way of the repaid record — that opens release, not seizure.
    w.vault.mark_repaid(&id);
    assert_eq!(
        w.vault.try_seize(&id, &treasury, &quote()),
        Err(Ok(Error::NoRecordedDefault))
    );
    assert_eq!(w.token.balance(&treasury), 0);
}

#[test]
fn a_stale_feed_refuses_to_lock_or_seize() {
    let env = Env::default();
    let w = setup(&env);
    let id = loan_id(&env);

    // One second past the published window.
    let stale = (NOW - MAX_PRICE_AGE_SECS - 1) * 1000;
    w.feed.set(&FEED_PRICE, &stale, &FEED_DECIMALS);
    assert_eq!(
        w.vault
            .try_lock(&w.depositor, &id, &REQUIRED_STROOPS, &PRINCIPAL, &quote()),
        Err(Ok(Error::PriceStale))
    );
    // A refused lock moves nothing.
    assert_eq!(w.token.balance(&w.depositor), MINTED);

    // The same gate stands at the other end of the loan's life.
    w.feed.set(&FEED_PRICE, &(NOW * 1000), &FEED_DECIMALS);
    lock(&w, &id);
    w.vault.mark_defaulted(&id);
    w.feed.set(&FEED_PRICE, &stale, &FEED_DECIMALS);
    let treasury = Address::generate(&env);
    assert_eq!(
        w.vault.try_seize(&id, &treasury, &quote()),
        Err(Ok(Error::PriceStale))
    );
    assert_eq!(w.token.balance(&treasury), 0);
}

#[test]
fn an_out_of_band_quote_is_refused() {
    let env = Env::default();
    let w = setup(&env);
    let id = loan_id(&env);

    // $0.44 against a feed saying $0.40 — 1000 bps out, twice the band.
    let inflated = Quote {
        usd_per_xlm_e8: 44_000_000,
        php_per_xlm_centavos: 2_200,
        ..quote()
    };
    assert_eq!(
        w.vault
            .try_lock(&w.depositor, &id, &REQUIRED_STROOPS, &PRINCIPAL, &inflated),
        Err(Ok(Error::PriceOutOfBand))
    );
    assert_eq!(w.token.balance(&w.depositor), MINTED);
}

#[test]
fn a_quote_at_the_edge_of_the_band_still_passes() {
    let env = Env::default();
    let w = setup(&env);
    // 500 bps exactly = the published band, so this is accepted; one bps
    // further is not. Both sides of the same line.
    let edge = Quote { usd_per_xlm_e8: 42_000_000, php_per_xlm_centavos: 2_100, ..quote() };
    let over = Quote { usd_per_xlm_e8: 42_004_000, php_per_xlm_centavos: 2_100, ..quote() };
    assert_eq!(
        w.vault.try_lock(
            &w.depositor,
            &BytesN::from_array(&env, &[1u8; 16]),
            &REQUIRED_STROOPS,
            &PRINCIPAL,
            &over
        ),
        Err(Ok(Error::PriceOutOfBand))
    );
    w.vault.lock(
        &w.depositor,
        &BytesN::from_array(&env, &[2u8; 16]),
        &REQUIRED_STROOPS,
        &PRINCIPAL,
        &edge,
    );
}

#[test]
fn a_feed_with_nothing_to_say_fails_closed() {
    let env = Env::default();
    let w = setup(&env);
    w.feed.silence();

    assert_eq!(
        w.vault.try_lock(
            &w.depositor,
            &loan_id(&env),
            &REQUIRED_STROOPS,
            &PRINCIPAL,
            &quote()
        ),
        Err(Ok(Error::PriceUnavailable))
    );
    assert_eq!(w.token.balance(&w.depositor), MINTED);
}

// ---- the happy paths -------------------------------------------------------

#[test]
fn repaid_then_released_returns_funds_to_the_depositor() {
    let env = Env::default();
    let w = setup(&env);
    let id = loan_id(&env);

    lock(&w, &id);
    assert_eq!(w.token.balance(&w.depositor), MINTED - REQUIRED_STROOPS);
    let position = w.vault.get_lock(&id).unwrap();
    assert_eq!(position.amount, REQUIRED_STROOPS);
    assert_eq!(position.principal_centavos, PRINCIPAL);
    assert_eq!(position.ratio_bps, RATIO_BPS);
    // The price it was struck at is pinned with the coins, feed included.
    assert_eq!(position.quote.php_per_xlm_centavos, PHP_PER_XLM);
    assert_eq!(position.feed.feed_usd_per_xlm_e8, USD_PER_XLM_E8);
    assert_eq!(position.feed.feed_at, NOW);

    w.vault.mark_repaid(&id);
    assert_eq!(w.vault.get_lock(&id).unwrap().state, LoanState::Repaid);

    w.vault.release(&id);
    assert_eq!(w.token.balance(&w.depositor), MINTED);
    assert!(w.vault.get_lock(&id).is_none());
}

#[test]
fn recorded_default_then_seizure_sends_funds_to_the_treasury() {
    let env = Env::default();
    let w = setup(&env);
    let treasury = Address::generate(&env);
    let id = loan_id(&env);

    lock(&w, &id);
    w.vault.mark_defaulted(&id);
    let recorded = w.vault.get_lock(&id).unwrap();
    assert_eq!(recorded.state, LoanState::Defaulted);
    assert_eq!(recorded.state_at, NOW);

    // XLM has fallen to $0.30 by the time the default is worked out; the
    // seizure is priced at the checked quote of the day, not at issuance.
    let fallen = Quote {
        usd_per_xlm_e8: 30_000_000,
        php_per_xlm_centavos: 1_500,
        php_per_usd_centavos: PHP_PER_USD,
    };
    w.feed.set(&30_000_000_000_000, &(NOW * 1000), &FEED_DECIMALS);
    w.vault.seize(&id, &treasury, &fallen);

    assert_eq!(w.token.balance(&treasury), REQUIRED_STROOPS);
    assert!(w.vault.get_lock(&id).is_none());
}

// ---- the edges -------------------------------------------------------------

#[test]
fn collateral_below_the_ratio_is_refused() {
    let env = Env::default();
    let w = setup(&env);
    let id = loan_id(&env);

    // One stroop short of 120% of the principal.
    assert_eq!(
        w.vault.try_lock(
            &w.depositor,
            &id,
            &(REQUIRED_STROOPS - 1),
            &PRINCIPAL,
            &quote()
        ),
        Err(Ok(Error::InsufficientCollateral))
    );
    assert_eq!(w.token.balance(&w.depositor), MINTED);
    // And exactly 120% is enough — the boundary is inclusive.
    lock(&w, &id);
}

#[test]
fn a_peso_rate_its_own_legs_do_not_support_is_refused() {
    let env = Env::default();
    let w = setup(&env);

    // The USD leg is inside the band, but ₱25.00/XLM is not what $0.40 at
    // ₱50.00/USD comes to — the peso number is unsupported by the legs it
    // was declared to be derived from.
    let inconsistent = Quote { php_per_xlm_centavos: 2_500, ..quote() };
    assert_eq!(
        w.vault.try_lock(
            &w.depositor,
            &loan_id(&env),
            &REQUIRED_STROOPS,
            &PRINCIPAL,
            &inconsistent
        ),
        Err(Ok(Error::InvalidQuote))
    );
}

#[test]
fn duplicate_lock_for_same_loan_is_refused() {
    let env = Env::default();
    let w = setup(&env);
    let id = loan_id(&env);

    lock(&w, &id);
    assert_eq!(
        w.vault
            .try_lock(&w.depositor, &id, &REQUIRED_STROOPS, &PRINCIPAL, &quote()),
        Err(Ok(Error::LockExists))
    );
}

#[test]
fn an_outcome_is_recorded_once() {
    let env = Env::default();
    let w = setup(&env);
    let id = loan_id(&env);

    lock(&w, &id);
    w.vault.mark_repaid(&id);
    // A repayment cannot be turned into a default after the fact.
    assert_eq!(w.vault.try_mark_defaulted(&id), Err(Ok(Error::NotActive)));
    assert_eq!(w.vault.try_mark_repaid(&id), Err(Ok(Error::NotActive)));
}

#[test]
fn the_ratio_cannot_be_configured_below_full_cover() {
    let env = Env::default();
    let w = setup(&env);
    let asset = Asset::Other(Symbol::new(&env, "XLM"));

    assert_eq!(
        w.vault
            .try_configure(&w.feed.address, &asset, &FEED_TIME_DIVISOR, &9_999),
        Err(Ok(Error::InvalidConfig))
    );
    // A recalibration upward is allowed — the ratio is a policy parameter.
    w.vault
        .configure(&w.feed.address, &asset, &FEED_TIME_DIVISOR, &15_000);
    assert_eq!(w.vault.get_config().collateral_ratio_bps, 15_000);
}

#[test]
fn a_pinned_position_keeps_the_ratio_it_was_struck_at() {
    let env = Env::default();
    let w = setup(&env);
    let id = loan_id(&env);
    lock(&w, &id);

    w.vault.configure(
        &w.feed.address,
        &Asset::Other(Symbol::new(&env, "XLM")),
        &FEED_TIME_DIVISOR,
        &15_000,
    );
    assert_eq!(w.vault.get_lock(&id).unwrap().ratio_bps, RATIO_BPS);
}

// ---- authorization ---------------------------------------------------------

#[test]
fn every_exit_and_every_record_requires_admin_auth() {
    let env = Env::default();
    let w = setup(&env);
    let id = loan_id(&env);
    lock(&w, &id);
    let treasury = Address::generate(&env);

    // From here, only explicitly-listed auths count: none. Every call that
    // could move coins, or open the door to moving them, must fail auth.
    env.set_auths(&[]);
    assert!(w.vault.try_mark_repaid(&id).is_err());
    assert!(w.vault.try_mark_defaulted(&id).is_err());
    assert!(w.vault.try_release(&id).is_err());
    assert!(w.vault.try_seize(&id, &treasury, &quote()).is_err());
    assert!(
        w.vault
            .try_configure(
                &w.feed.address,
                &Asset::Other(Symbol::new(&env, "XLM")),
                &FEED_TIME_DIVISOR,
                &15_000
            )
            .is_err()
    );
    assert_eq!(w.token.balance(&treasury), 0);
    assert_eq!(w.vault.get_lock(&id).unwrap().state, LoanState::Active);
}

#[test]
fn the_admin_cannot_redirect_a_release() {
    let env = Env::default();
    let w = setup(&env);
    let id = loan_id(&env);
    lock(&w, &id);
    w.vault.mark_repaid(&id);

    // `release` takes no destination at all: the depositor recorded at lock
    // time is the only place the coins can go, so there is nothing for a
    // compromised engine key to redirect.
    w.vault.release(&id);
    assert_eq!(w.token.balance(&w.depositor), MINTED);
    assert_eq!(w.token.balance(&w.admin), 0);
}
