//! Live XLM price reads from independent public feeds.
//!
//! This module is I/O only: it asks several unrelated providers what XLM is
//! worth and hands back whatever answered, as integers. It never decides a
//! rate — agreeing on one number out of many is pure arithmetic and lives in
//! `api::lending::domain` (blueprint §3 dependency rule), and pinning it to a
//! loan lives in `api::lending::pricing`.
//!
//! Two legs, because no free provider quotes XLM/PHP with real depth
//! (SOW §3.10, "Price source and oracle bounds"): the crypto venues quote
//! XLM/USD, the fiat feeds quote USD/PHP, and the product of the two is the
//! derived rate that gets recorded with the loan. CoinGecko is asked for the
//! pair directly so at least one candidate never depends on the fiat leg.
//!
//! Every provider is queried concurrently and independently — one being
//! down, rate-limited or nonsense costs its own reading and nothing else.
//!
//! Env: XLM_PRICE_SOURCES (comma list, narrows the default set — e.g.
//! "coingecko,kraken"), XLM_PRICE_TIMEOUT_SECS (default 8).

use std::sync::OnceLock;

use serde_json::Value;

/// Fixed-point scale for every rate in this module: 1e8, so a price is an
/// integer count of hundred-millionths. No float ever holds a number we act
/// on — provider JSON is re-read from its text form and parsed by string
/// math, the same rule `stellar::parse_stroops` follows for on-chain amounts.
pub const RATE_SCALE: i64 = 100_000_000;

/// One provider's answer for one pair.
#[derive(Clone)]
pub struct SourceRate {
    pub name: &'static str,
    /// Scaled by RATE_SCALE.
    pub scaled: i64,
}

/// A provider that didn't answer usably, kept so the reason can be logged
/// and shown on the proof page rather than vanishing into a median.
#[derive(Clone)]
pub struct Failure {
    pub name: &'static str,
    pub reason: &'static str,
}

/// Everything the feeds said in one round. Empty vectors are normal —
/// the caller decides whether what survived is enough to price a loan.
#[derive(Default)]
pub struct Reading {
    /// PHP per 1 XLM, quoted directly by the provider.
    pub direct_php: Vec<SourceRate>,
    /// USD per 1 XLM, the crypto-venue leg.
    pub usd_per_xlm: Vec<SourceRate>,
    /// PHP per 1 USD, the fiat leg the crypto quotes are carried across on.
    pub php_per_usd: Vec<SourceRate>,
    pub failures: Vec<Failure>,
}

fn http() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        let secs = std::env::var("XLM_PRICE_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|s| (1..=30).contains(s))
            .unwrap_or(8);
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(secs))
            // A price read is on the apply path; a provider that hangs its
            // connection must not hold a borrower's application open.
            .connect_timeout(std::time::Duration::from_secs(4))
            .user_agent("PrimeLendRow/1.0 (+lending engine price oracle)")
            .build()
            .expect("reqwest client")
    })
}

/// Operators can narrow the set without a redeploy of new code; an unset or
/// empty variable means "all of them".
fn enabled(name: &str) -> bool {
    static ALLOW: OnceLock<Option<Vec<String>>> = OnceLock::new();
    let allow = ALLOW.get_or_init(|| {
        let raw = std::env::var("XLM_PRICE_SOURCES").ok()?;
        let list: Vec<String> = raw
            .split(',')
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        (!list.is_empty()).then_some(list)
    });
    match allow {
        Some(list) => list.iter().any(|s| s == name),
        None => true,
    }
}

/// A decimal string -> integer scaled by RATE_SCALE, string math only.
/// Anything that isn't plain unsigned decimal (signs, exponents, blanks) is
/// refused rather than coerced — a malformed quote must drop out of the
/// median, never enter it as a zero.
fn parse_scaled(raw: &str) -> Result<i64, &'static str> {
    let raw = raw.trim();
    let (whole, frac) = raw.split_once('.').unwrap_or((raw, ""));
    if whole.is_empty() || whole.len() > 9 || !whole.chars().all(|c| c.is_ascii_digit()) {
        return Err("price out of shape");
    }
    if !frac.chars().all(|c| c.is_ascii_digit()) {
        return Err("price out of shape");
    }
    let whole: i64 = whole.parse().map_err(|_| "price out of shape")?;
    // Kept to 1e-8; a provider quoting finer than that is not telling us
    // anything the 120% rule can act on. Truncation, not rounding — the
    // single rounding site is domain::round_half_even, downstream.
    let keep = &frac[..frac.len().min(8)];
    let frac_scaled = if keep.is_empty() {
        0
    } else {
        keep.parse::<i64>().map_err(|_| "price out of shape")?
            * 10i64.pow(8 - keep.len() as u32)
    };
    whole
        .checked_mul(RATE_SCALE)
        .and_then(|w| w.checked_add(frac_scaled))
        .ok_or("price out of shape")
}

/// Providers hand prices back as either JSON strings or JSON numbers.
/// `Number::to_string` is the shortest round-trip text of the value, so the
/// number path still ends up in `parse_scaled` as text.
fn scaled_from_json(v: Option<&Value>) -> Result<i64, &'static str> {
    match v {
        Some(Value::String(s)) => parse_scaled(s),
        Some(Value::Number(n)) => parse_scaled(&n.to_string()),
        _ => Err("price field missing"),
    }
}

/// A reading outside the band is a provider malfunction, not a market move:
/// XLM has never been near $100, and PHP has never been near 10 or 200 to
/// the dollar. Bounds are deliberately wide — this rejects broken feeds, and
/// the deviation check downstream is what rejects merely wrong ones.
fn in_band(scaled: i64, lo: i64, hi: i64) -> Result<i64, &'static str> {
    if scaled >= lo && scaled <= hi {
        Ok(scaled)
    } else {
        Err("price outside sanity bounds")
    }
}

const USD_PER_XLM_MIN: i64 = RATE_SCALE / 10_000; // $0.0001
const USD_PER_XLM_MAX: i64 = RATE_SCALE * 100; // $100
const PHP_PER_USD_MIN: i64 = RATE_SCALE * 10;
const PHP_PER_USD_MAX: i64 = RATE_SCALE * 200;
const PHP_PER_XLM_MIN: i64 = RATE_SCALE / 100;
const PHP_PER_XLM_MAX: i64 = RATE_SCALE * 10_000;

async fn get_json(url: &str) -> Result<Value, &'static str> {
    let res = http().get(url).send().await.map_err(|e| {
        tracing::warn!("price feed {url}: {e}");
        "feed unreachable"
    })?;
    if !res.status().is_success() {
        tracing::warn!("price feed {url}: HTTP {}", res.status());
        return Err("feed refused the request");
    }
    res.json().await.map_err(|e| {
        tracing::warn!("price feed {url} body: {e}");
        "feed sent an unreadable body"
    })
}

// ---- crypto venues: USD per XLM -------------------------------------------

async fn binance() -> Result<i64, &'static str> {
    let v = get_json("https://api.binance.com/api/v3/ticker/price?symbol=XLMUSDT").await?;
    in_band(
        scaled_from_json(v.get("price"))?,
        USD_PER_XLM_MIN,
        USD_PER_XLM_MAX,
    )
}

async fn kraken() -> Result<i64, &'static str> {
    let v = get_json("https://api.kraken.com/0/public/Ticker?pair=XLMUSD").await?;
    // Kraken keys the result by its own asset naming (XXLMZUSD); take the one
    // pair it returned rather than hard-coding a name that may be renamed.
    let pair = v
        .get("result")
        .and_then(|r| r.as_object())
        .and_then(|o| o.values().next())
        .ok_or("unexpected body")?;
    // "c" is [last trade price, lot volume].
    let last = pair.get("c").and_then(|c| c.get(0));
    in_band(scaled_from_json(last)?, USD_PER_XLM_MIN, USD_PER_XLM_MAX)
}

async fn coinbase() -> Result<i64, &'static str> {
    let v = get_json("https://api.coinbase.com/v2/prices/XLM-USD/spot").await?;
    let amount = v.get("data").and_then(|d| d.get("amount"));
    in_band(scaled_from_json(amount)?, USD_PER_XLM_MIN, USD_PER_XLM_MAX)
}

// ---- direct pair ----------------------------------------------------------

async fn coingecko() -> Result<i64, &'static str> {
    let v = get_json(
        "https://api.coingecko.com/api/v3/simple/price?ids=stellar&vs_currencies=php",
    )
    .await?;
    let php = v.get("stellar").and_then(|s| s.get("php"));
    in_band(scaled_from_json(php)?, PHP_PER_XLM_MIN, PHP_PER_XLM_MAX)
}

// ---- fiat leg: PHP per USD ------------------------------------------------
//
// Both fiat feeds publish DAILY, not tick-by-tick (Frankfurter mirrors ECB
// reference rates, so its date sits on the last banking day). That is fine
// for what it is used for — USD/PHP does not move 5% over a weekend, and the
// deviation band is what would catch it if it ever did — but it is why the
// staleness window in `pricing` guards the agreement's age, not the feed's.

async fn er_api() -> Result<i64, &'static str> {
    let v = get_json("https://open.er-api.com/v6/latest/USD").await?;
    let php = v.get("rates").and_then(|r| r.get("PHP"));
    in_band(scaled_from_json(php)?, PHP_PER_USD_MIN, PHP_PER_USD_MAX)
}

async fn frankfurter() -> Result<i64, &'static str> {
    // .dev/v1 rather than .app/latest: the latter 301s, and a price read
    // should not depend on redirect-following staying enabled.
    let v = get_json("https://api.frankfurter.dev/v1/latest?base=USD&symbols=PHP").await?;
    let php = v.get("rates").and_then(|r| r.get("PHP"));
    in_band(scaled_from_json(php)?, PHP_PER_USD_MIN, PHP_PER_USD_MAX)
}

/// Ask every enabled provider at once and collect what came back.
/// Never fails: an empty `Reading` is a legitimate answer, and refusing to
/// lend on one is the caller's decision (fail-closed lives in `pricing`).
pub async fn read() -> Reading {
    let mut out = Reading::default();

    macro_rules! leg {
        ($name:expr, $fut:expr, $bucket:ident) => {
            if enabled($name) {
                match $fut {
                    Ok(scaled) => out.$bucket.push(SourceRate { name: $name, scaled }),
                    Err(reason) => out.failures.push(Failure { name: $name, reason }),
                }
            }
        };
    }

    // One round trip's worth of wall clock for all six, not six in a row.
    let (gecko, bin, krak, cb, er, frank) = tokio::join!(
        coingecko(),
        binance(),
        kraken(),
        coinbase(),
        er_api(),
        frankfurter(),
    );

    leg!("coingecko", gecko, direct_php);
    leg!("binance", bin, usd_per_xlm);
    leg!("kraken", krak, usd_per_xlm);
    leg!("coinbase", cb, usd_per_xlm);
    leg!("er-api", er, php_per_usd);
    leg!("frankfurter", frank, php_per_usd);

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_provider_decimals_by_string_math() {
        assert_eq!(parse_scaled("1"), Ok(100_000_000));
        assert_eq!(parse_scaled("0.39120000"), Ok(39_120_000));
        assert_eq!(parse_scaled("58.5"), Ok(5_850_000_000));
        assert_eq!(parse_scaled(" 22.83 "), Ok(2_283_000_000));
        // finer than 1e-8 is truncated, not rounded
        assert_eq!(parse_scaled("0.123456789"), Ok(12_345_678));
    }

    #[test]
    fn refuses_anything_not_plain_decimal() {
        for bad in ["-1", "1.2e-5", "", "abc", "1.2.3", "1234567890"] {
            assert!(parse_scaled(bad).is_err(), "{bad} should not parse");
        }
    }

    #[test]
    fn reads_both_json_string_and_number_prices() {
        let v: Value = serde_json::json!({ "s": "0.3912", "n": 22.83, "b": true });
        assert_eq!(scaled_from_json(v.get("s")), Ok(39_120_000));
        assert_eq!(scaled_from_json(v.get("n")), Ok(2_283_000_000));
        assert!(scaled_from_json(v.get("b")).is_err());
        assert!(scaled_from_json(None).is_err());
    }

    #[test]
    fn sanity_bounds_reject_broken_feeds() {
        assert!(in_band(0, USD_PER_XLM_MIN, USD_PER_XLM_MAX).is_err());
        assert!(in_band(RATE_SCALE * 5_000, USD_PER_XLM_MIN, USD_PER_XLM_MAX).is_err());
        assert!(in_band(RATE_SCALE / 2, USD_PER_XLM_MIN, USD_PER_XLM_MAX).is_ok());
    }
}
