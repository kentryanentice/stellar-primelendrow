//! The XLM→PHP rate the 120%/110% collateral rules value against — read from
//! several independent public feeds rather than typed in by an admin.
//!
//! Three layers meet here and nowhere else:
//!   infra::oracle  asks the feeds (I/O, no decisions)
//!   domain         agrees on one number out of many (pure, tested)
//!   this file      caches it, records it, and decides who may act on it
//!
//! The rule the SOW commits to (§3.10) is fail-closed: a loan is priced only
//! when `MIN_SOURCES` independent feeds agree within `MAX_DEVIATION_BPS`, and
//! only while that agreement is younger than `MAX_AGE_SECS`. When no feed
//! agrees, issuance is REFUSED — `for_issuance` returns 503. Screens are not
//! money, so `for_display` degrades instead: it falls back to the last rate
//! on record and flags itself `live: false` so the UI can say so.
//!
//! Every accepted quote carries its own evidence — which feed said what, and
//! how far each sat from the agreed number — because the proof page has to
//! show "the price used at each collateral movement, with timestamp".

use std::sync::OnceLock;
use std::sync::atomic::{AtomicI64, Ordering};

use axum::http::StatusCode;
use chrono::Utc;
use serde::Serialize;
use sqlx::PgPool;
use tokio::sync::{Mutex, RwLock};

use super::domain;
use super::policy;
use crate::api::users::shared::E;
use crate::infra::oracle;

/// An agreed quote is reused without re-asking the feeds for this long.
const FRESH_SECS: i64 = 60;
/// The staleness window: past this, a quote can no longer price a loan even
/// if it is the newest thing we have.
const MAX_AGE_SECS: i64 = 900;
/// Independent feeds that must still agree after outliers are dropped. One
/// feed can be wrong in a way nothing contradicts, so one is never enough.
const MIN_SOURCES: usize = 2;
/// How far a feed may sit from the median and still count: 5%.
const MAX_DEVIATION_BPS: i64 = 500;
/// The agreed rate is appended to fx_rates at most this often — enough of a
/// trail to render a page when every feed is down, without a row per minute.
const PERSIST_MIN_SECS: i64 = 300;

/// One feed's contribution to an agreed price, kept for the record.
#[derive(Clone, Serialize)]
pub struct PricedSource {
    pub name: String,
    /// What this feed alone implied, in whole centavos per XLM.
    pub centavos_per_xlm: i64,
    /// "XLM/PHP" (quoted directly) or "XLM/USD x USD/PHP" (derived).
    pub leg: &'static str,
    /// Distance from the agreed rate, in basis points. Feeds beyond
    /// MAX_DEVIATION_BPS were excluded from the median that produced it.
    pub deviation_bps: i64,
    pub used: bool,
}

/// A feed that didn't answer usably. Recorded rather than dropped: "two of
/// five feeds were down" is exactly the sort of thing a reviewer should be
/// able to see behind a price.
#[derive(Clone, Serialize)]
pub struct PricedFailure {
    pub name: String,
    pub reason: String,
}

#[derive(Clone, Serialize)]
pub struct Priced {
    /// Whole centavos one XLM is worth — the only number money math touches.
    pub centavos_per_xlm: i64,
    /// Unix seconds the feeds were read.
    pub as_of: i64,
    /// Plain-language description of how the number was reached.
    pub method: String,
    /// The USD/PHP leg the derived quotes crossed, in centavos per USD, when
    /// one was used. Null when every contributing feed quoted XLM/PHP itself.
    pub usd_php_centavos: Option<i64>,
    /// The XLM/USD leg alone, scaled by `oracle::RATE_SCALE` (1e8) — the only
    /// part of this price a Stellar feed can check, so it is what the vault
    /// contract is handed to measure against Reflector. Null when no crypto
    /// venue answered, which is why `for_issuance` then refuses: a price the
    /// contract cannot check is a price it will not act on.
    pub usd_per_xlm_e8: Option<i64>,
    pub sources: Vec<PricedSource>,
    pub failures: Vec<PricedFailure>,
    /// false = no feed agreed and this is the last rate on record. Display
    /// only — never enough to issue a loan against.
    pub live: bool,
}

fn cache() -> &'static RwLock<Option<Priced>> {
    static C: OnceLock<RwLock<Option<Priced>>> = OnceLock::new();
    C.get_or_init(|| RwLock::new(None))
}

/// Serializes refreshes so a burst of borrowers costs the feeds one round of
/// requests, not one round each. Held across the HTTP call — which is why
/// nothing in this module may ever be invoked inside a database transaction.
fn refresh_gate() -> &'static Mutex<()> {
    static G: OnceLock<Mutex<()>> = OnceLock::new();
    G.get_or_init(|| Mutex::new(()))
}

/// When the feeds were last *asked*, agreement or not. The cache only records
/// successes, so without this a total outage would have every request in turn
/// pay a full round of timeouts behind the gate.
fn last_attempt() -> &'static AtomicI64 {
    static A: OnceLock<AtomicI64> = OnceLock::new();
    A.get_or_init(|| AtomicI64::new(0))
}

/// What to serve when this round produced no agreement: a recent one still
/// stands inside the staleness window, anything older fails closed.
fn hold_or_refuse(cached: Option<Priced>, now: i64) -> Result<Priced, E> {
    cached.filter(|p| now - p.as_of <= MAX_AGE_SECS).ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "XLM pricing is unavailable — no independent price feeds agree right now",
    ))
}

/// Turn a round of feed answers into one price, or None to refuse.
fn aggregate(reading: oracle::Reading, as_of: i64) -> Option<Priced> {
    // The fiat leg first: the crypto venues quote dollars, and the median of
    // the fiat feeds is what carries those quotes into pesos.
    let usd_php_scaled = domain::median(
        &reading.php_per_usd.iter().map(|s| s.scaled).collect::<Vec<_>>(),
    );
    // The dollar leg on its own, kept whole: the vault contract checks this
    // number against Reflector, so it travels beside the peso rate rather
    // than being folded into it and lost.
    let usd_per_xlm_scaled = domain::median(
        &reading.usd_per_xlm.iter().map(|s| s.scaled).collect::<Vec<_>>(),
    );

    let mut candidates: Vec<(String, i64, &'static str)> = Vec::new();
    for s in &reading.direct_php {
        candidates.push((
            s.name.to_string(),
            domain::scaled_to_centavos(s.scaled),
            "XLM/PHP",
        ));
    }
    if let Some(fx) = usd_php_scaled {
        for s in &reading.usd_per_xlm {
            candidates.push((
                s.name.to_string(),
                domain::scaled_to_centavos(domain::derive_php_per_xlm(s.scaled, fx)),
                "XLM/USD x USD/PHP",
            ));
        }
    }

    let quotes: Vec<i64> = candidates.iter().map(|(_, c, _)| *c).collect();
    let agreed = domain::agree_on_rate(&quotes, MIN_SOURCES, MAX_DEVIATION_BPS)?;

    let sources: Vec<PricedSource> = candidates
        .into_iter()
        .map(|(name, centavos, leg)| {
            let deviation_bps = (centavos as i128 - agreed as i128).abs() * 10_000
                / (agreed.max(1) as i128);
            PricedSource {
                name,
                centavos_per_xlm: centavos,
                leg,
                deviation_bps: deviation_bps as i64,
                used: deviation_bps as i64 <= MAX_DEVIATION_BPS,
            }
        })
        .collect();

    let used = sources.iter().filter(|s| s.used).count();
    Some(Priced {
        centavos_per_xlm: agreed,
        as_of,
        method: format!(
            "median of {used} agreeing feed{} out of {} read",
            if used == 1 { "" } else { "s" },
            sources.len()
        ),
        usd_php_centavos: usd_php_scaled.map(domain::scaled_to_centavos),
        usd_per_xlm_e8: usd_per_xlm_scaled,
        sources,
        failures: reading
            .failures
            .into_iter()
            .map(|f| PricedFailure { name: f.name.to_string(), reason: f.reason.to_string() })
            .collect(),
        live: true,
    })
}

/// Append the agreed rate to the append-only history, throttled. Best effort
/// by design: a price that cannot be filed is still a price we agreed on,
/// and refusing the loan over a logging failure would be the wrong trade.
async fn persist(pool: &PgPool, priced: &Priced) {
    let last: Result<Option<i64>, _> = sqlx::query_scalar(
        "SELECT created_at FROM public.fx_rates
          WHERE base = 'XLM' AND quote = 'PHP' AND source = 'oracle'
          ORDER BY created_at DESC, id DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await;
    match last {
        Ok(Some(at)) if priced.as_of - at < PERSIST_MIN_SECS => return,
        Err(e) => {
            tracing::warn!("fx history read: {e}");
            return;
        }
        _ => {}
    }

    let sources = serde_json::to_value(&priced.sources).unwrap_or(serde_json::Value::Null);
    if let Err(e) = sqlx::query(
        "INSERT INTO public.fx_rates (centavos_per_xlm, source, method, sources, created_at)
         VALUES ($1, 'oracle', $2, $3, $4)",
    )
    .bind(priced.centavos_per_xlm)
    .bind(&priced.method)
    .bind(sources)
    .bind(priced.as_of)
    .execute(pool)
    .await
    {
        tracing::warn!("fx history write: {e}");
    }
}

/// The current agreed price, refreshing the feeds when the cached one has
/// aged out. Never call this inside a transaction — it holds a mutex across
/// network I/O, and a database round trip must never wait behind Binance.
async fn current(pool: &PgPool) -> Result<Priced, E> {
    if let Some(p) = cache().read().await.clone() {
        if Utc::now().timestamp() - p.as_of < FRESH_SECS {
            return Ok(p);
        }
    }

    let _gate = refresh_gate().lock().await;
    // Someone may have refreshed while we queued on the gate.
    let cached = cache().read().await.clone();
    let now = Utc::now().timestamp();
    if let Some(p) = &cached {
        if now - p.as_of < FRESH_SECS {
            return Ok(p.clone());
        }
    }
    // One attempt per window even when it fails, so an outage costs one round
    // of timeouts rather than one per waiting request.
    if now - last_attempt().load(Ordering::Relaxed) < FRESH_SECS {
        return hold_or_refuse(cached, now);
    }
    last_attempt().store(now, Ordering::Relaxed);

    let reading = oracle::read().await;
    let now = Utc::now().timestamp();
    let failed: Vec<&str> = reading.failures.iter().map(|f| f.name).collect();

    match aggregate(reading, now) {
        Some(priced) => {
            if !failed.is_empty() {
                tracing::warn!(rate = priced.centavos_per_xlm, ?failed, "priced XLM with feeds missing");
            } else {
                tracing::info!(rate = priced.centavos_per_xlm, method = %priced.method, "priced XLM");
            }
            *cache().write().await = Some(priced.clone());
            persist(pool, &priced).await;
            Ok(priced)
        }
        // No agreement this round. A recent agreement is still inside the
        // staleness window and may stand; anything older fails closed.
        None => {
            let held = hold_or_refuse(cached, now);
            match &held {
                Ok(p) => tracing::warn!(?failed, age = now - p.as_of, "no feed agreement — holding the last agreed rate"),
                Err(_) => tracing::error!(?failed, "no XLM price feeds agreed and nothing recent to hold"),
            }
            held
        }
    }
}

impl Priced {
    /// The two legs the vault contract is handed with a lock: the XLM/USD it
    /// measures against Reflector, and the USD/PHP it records beside it. Both
    /// or neither — a peso rate with no dollar leg behind it is one the
    /// contract has no way to check.
    pub fn checkable_legs(&self) -> Option<(i64, i64)> {
        Some((self.usd_per_xlm_e8?, self.usd_php_centavos?))
    }
}

/// The rate a loan may be priced at. Fails closed: an unavailable or
/// disagreeing set of feeds refuses the loan rather than issuing it at a
/// price nobody can vouch for — and so does a price the vault contract
/// couldn't check, since a lock submitted without a dollar leg is a lock the
/// chain will reject anyway.
pub async fn for_issuance(pool: &PgPool) -> Result<Priced, E> {
    let priced = current(pool).await?;
    if !priced.live
        || Utc::now().timestamp() - priced.as_of > MAX_AGE_SECS
        || priced.checkable_legs().is_none()
    {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "XLM pricing is unavailable — no independent price feeds agree right now",
        ));
    }
    Ok(priced)
}

/// The rate a screen may show. Degrades instead of failing: with no live
/// agreement it returns the last rate on record, marked `live: false` so the
/// UI can label it. Nothing that moves money may use this.
pub async fn for_display(pool: &PgPool) -> Result<Priced, E> {
    if let Ok(priced) = current(pool).await {
        return Ok(priced);
    }
    let (centavos_per_xlm, as_of) = policy::last_recorded_fx(pool).await?;
    Ok(Priced {
        centavos_per_xlm,
        as_of,
        method: "last recorded rate — no live feed agreed".to_string(),
        usd_php_centavos: None,
        usd_per_xlm_e8: None,
        sources: Vec::new(),
        failures: Vec::new(),
        live: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::oracle::{Failure, Reading, SourceRate};

    const SCALE: i64 = oracle::RATE_SCALE;

    fn reading(direct: &[(&'static str, f64)], usd: &[(&'static str, f64)], fx: &[(&'static str, f64)]) -> Reading {
        // Test-only convenience: the production path never builds a rate from
        // a float — `oracle::parse_scaled` does it from the provider's text.
        let scale = |v: &f64| (v * SCALE as f64).round() as i64;
        Reading {
            direct_php: direct.iter().map(|(n, v)| SourceRate { name: n, scaled: scale(v) }).collect(),
            usd_per_xlm: usd.iter().map(|(n, v)| SourceRate { name: n, scaled: scale(v) }).collect(),
            php_per_usd: fx.iter().map(|(n, v)| SourceRate { name: n, scaled: scale(v) }).collect(),
            failures: vec![Failure { name: "kraken", reason: "feed unreachable" }],
        }
    }

    #[test]
    fn crosses_the_dollar_leg_and_agrees_with_the_direct_feed() {
        // ₱22.82 direct, and $0.39 x ₱58.50 = ₱22.815 -> ₱22.82 derived.
        let p = aggregate(
            reading(&[("coingecko", 22.82)], &[("binance", 0.39), ("coinbase", 0.3901)], &[("er-api", 58.5), ("frankfurter", 58.5)]),
            1_700_000_000,
        )
        .expect("three feeds agree");
        assert_eq!(p.centavos_per_xlm, 2282);
        assert_eq!(p.usd_php_centavos, Some(5850));
        // The dollar leg survives whole for the contract to check: the median
        // of $0.3900 and $0.3901, scaled 1e8.
        assert_eq!(p.usd_per_xlm_e8, Some(39_005_000));
        assert!(p.checkable_legs().is_some());
        assert_eq!(p.sources.len(), 3);
        assert!(p.sources.iter().all(|s| s.used));
        assert_eq!(p.failures.len(), 1);
        assert!(p.live);
    }

    #[test]
    fn a_lone_feed_is_never_enough() {
        // No fiat leg answered, so the dollar quotes cannot be crossed and
        // only the direct feed survives — below quorum, so: no price.
        assert!(aggregate(reading(&[("coingecko", 22.82)], &[("binance", 0.39)], &[]), 0).is_none());
    }

    #[test]
    fn a_price_with_no_dollar_leg_is_not_issuable() {
        // Two direct XLM/PHP feeds agree, so there IS a price — but no crypto
        // venue answered, so there is nothing the vault contract could check
        // it against, and `for_issuance` refuses on exactly this.
        let p = aggregate(reading(&[("coingecko", 22.82), ("kraken", 22.83)], &[], &[]), 0)
            .expect("two direct feeds agree");
        assert_eq!(p.centavos_per_xlm, 2282); // ₱22.825, half-to-even
        assert!(p.checkable_legs().is_none());
    }

    #[test]
    fn an_outlier_is_dropped_but_still_recorded() {
        let p = aggregate(
            reading(&[("coingecko", 22.82)], &[("binance", 0.39), ("coinbase", 9.0)], &[("er-api", 58.5)]),
            0,
        )
        .expect("two honest feeds still agree");
        assert_eq!(p.centavos_per_xlm, 2282);
        let outlier = p.sources.iter().find(|s| s.name == "coinbase").unwrap();
        assert!(!outlier.used);
        assert!(outlier.deviation_bps > MAX_DEVIATION_BPS);
    }

    /// Not part of the normal run — it talks to six live providers, and a
    /// test suite that fails because CoinGecko rate-limited is a bad test
    /// suite. Run it deliberately when touching a feed's parsing:
    ///   cargo test -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "hits live public price feeds"]
    async fn live_feeds_agree_on_a_real_rate() {
        let priced = aggregate(oracle::read().await, Utc::now().timestamp())
            .expect("at least two independent public feeds should agree");
        println!(
            "\n  agreed: PHP {}.{:02} per XLM — {}",
            priced.centavos_per_xlm / 100,
            priced.centavos_per_xlm % 100,
            priced.method
        );
        for s in &priced.sources {
            println!(
                "    {:<12} {:>7} centavos  {:<20} {:>4} bps  {}",
                s.name,
                s.centavos_per_xlm,
                s.leg,
                s.deviation_bps,
                if s.used { "used" } else { "DROPPED" }
            );
        }
        for f in &priced.failures {
            println!("    {:<12} unavailable: {}", f.name, f.reason);
        }
        assert!(priced.centavos_per_xlm > 0);
        assert!(priced.sources.iter().filter(|s| s.used).count() >= MIN_SOURCES);
    }

    #[test]
    fn total_disagreement_refuses_to_price() {
        assert!(
            aggregate(reading(&[("coingecko", 22.82)], &[("binance", 9.0)], &[("er-api", 58.5)]), 0)
                .is_none()
        );
    }
}
