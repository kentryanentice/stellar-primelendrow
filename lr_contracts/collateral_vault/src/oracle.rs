//! The SEP-40 price feed the vault checks the engine's quote against.
//!
//! The engine reads six public feeds off-chain and agrees one number
//! (`lr_engine/src/api/lending/pricing.rs`). That number is a *candidate*: it
//! is chosen by the platform, so on its own it is exactly the kind of
//! operator discretion this sprint exists to narrow. Before the vault acts on
//! it, the quote is measured against a public on-chain feed — Reflector, over
//! the SEP-40 interface — and the operation is refused if
//!
//!   * the feed has nothing to say (`PriceUnavailable`),
//!   * its newest point is older than [`MAX_PRICE_AGE_SECS`] (`PriceStale`), or
//!   * the quote sits further than [`MAX_DEVIATION_BPS`] from it
//!     (`PriceOutOfBand`).
//!
//! Both bounds are contract constants, not stored settings: the admin can
//! point the vault at a different feed, but cannot widen the window it is
//! judged in. The check therefore FAILS CLOSED — no usable feed means no lock
//! and no seizure, never an unchecked one (SOW §3.10).
//!
//! **What is checked, and what is not.** The feed quotes XLM in USD; the loan
//! is in pesos. Only the XLM/USD leg is verifiable on-chain, so that is what
//! is bounded here. The USD/PHP leg the engine crossed it through is recorded
//! with the lock but cannot be checked by any Stellar feed, and the caller
//! (`lock`) additionally requires that the peso rate it submits is consistent
//! with the two legs it declares. Within the band, the backend still chooses
//! the exact number — the contract narrows that discretion, it does not
//! eliminate it.

use soroban_sdk::{contractclient, contracttype, Address, Env, Symbol};

use crate::{Config, Error};

/// SEP-40's asset identifier: a Stellar asset by contract address, or an
/// off-chain symbol like `XLM` on Reflector's CEX/DEX feed.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Asset {
    Stellar(Address),
    Other(Symbol),
}

/// One point from the feed. `price` carries the feed's own `decimals()`, and
/// `timestamp` its own time unit — SEP-40 fixes neither, and Reflector's
/// deployments disagree with each other, which is why
/// [`Config::feed_time_divisor`] is configured per deployment rather than
/// assumed here. Verify with a real `lastprice` call before initializing.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceData {
    pub price: i128,
    pub timestamp: u64,
}

/// The slice of SEP-40 the vault needs. Anything implementing these two entry
/// points can back the vault, which is what makes the check testable against
/// a mock feed rather than only against a live network.
#[contractclient(name = "PriceFeedClient")]
// The trait exists to generate `PriceFeedClient`; nothing implements it here.
#[allow(dead_code)]
pub trait PriceFeed {
    fn decimals(env: Env) -> u32;
    fn lastprice(env: Env, asset: Asset) -> Option<PriceData>;
}

/// Every rate crossing this contract's boundary is an integer scaled by 1e8 —
/// the same fixed point `lr_engine`'s oracle parses provider text into, so the
/// number the engine agreed on is the number the contract judges, with no
/// re-scaling in between.
pub const QUOTE_SCALE: i128 = 100_000_000;

/// A feed point older than this cannot price a lock or a seizure. Matches the
/// engine's own staleness window (`pricing::MAX_AGE_SECS`, 15 minutes).
pub const MAX_PRICE_AGE_SECS: u64 = 900;

/// How far the submitted quote may sit from the feed: 5%, the same band the
/// engine drops outlying providers at.
pub const MAX_DEVIATION_BPS: i128 = 500;

/// A feed reporting more decimals than this is not something we can rescale
/// without overflowing; treated as unusable rather than guessed at.
const MAX_FEED_DECIMALS: u32 = 30;

/// What the feed said, kept so it can be stored with the position and shown
/// on the proof page beside the quote it approved.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Checked {
    /// The feed's price, normalized to [`QUOTE_SCALE`].
    pub feed_usd_per_xlm_e8: i128,
    /// The feed point's own time, in Unix seconds.
    pub feed_at: u64,
}

/// True when `value` sits further than `max_bps` from `reference`.
/// `reference` must be positive; both are plain integers on the same scale.
pub fn deviates(value: i128, reference: i128, max_bps: i128) -> bool {
    let diff = if value > reference { value - reference } else { reference - value };
    diff * 10_000 > reference * max_bps
}

/// Rescale a feed price carrying `decimals` places to [`QUOTE_SCALE`].
fn to_e8(price: i128, decimals: u32) -> Result<i128, Error> {
    if decimals > MAX_FEED_DECIMALS {
        return Err(Error::PriceUnavailable);
    }
    Ok(if decimals >= 8 {
        price / 10i128.pow(decimals - 8)
    } else {
        price * 10i128.pow(8 - decimals)
    })
}

/// Judge `quoted_e8` (USD per XLM, scaled 1e8) against the configured feed.
/// Returns what the feed said, or the reason the vault must refuse to act.
pub fn check(env: &Env, cfg: &Config, quoted_e8: i128) -> Result<Checked, Error> {
    if quoted_e8 <= 0 {
        return Err(Error::InvalidQuote);
    }

    let feed = PriceFeedClient::new(env, &cfg.oracle);
    let point = feed.lastprice(&cfg.asset).ok_or(Error::PriceUnavailable)?;

    let feed_usd_per_xlm_e8 = to_e8(point.price, feed.decimals())?;
    if feed_usd_per_xlm_e8 <= 0 {
        return Err(Error::PriceUnavailable);
    }

    let feed_at = point.timestamp / cfg.feed_time_divisor.max(1);
    // saturating: a feed point stamped slightly ahead of the ledger clock is
    // fresh, not an underflow.
    if env.ledger().timestamp().saturating_sub(feed_at) > MAX_PRICE_AGE_SECS {
        return Err(Error::PriceStale);
    }

    if deviates(quoted_e8, feed_usd_per_xlm_e8, MAX_DEVIATION_BPS) {
        return Err(Error::PriceOutOfBand);
    }

    Ok(Checked { feed_usd_per_xlm_e8, feed_at })
}
