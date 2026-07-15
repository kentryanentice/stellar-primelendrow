//! PURE lending rules — no DB, no clock, no HTTP. Every function here is a
//! plain function of integers and the rulebook, unit-tested with plain
//! numbers at the bottom of the file. If it can't be tested that way, it
//! does not belong in this file (blueprint §3 dependency rule).
//!
//! Money is whole centavos in i64; intermediate products use i128 so a cap
//! times a percentage can never overflow on the way to a valid result.

use super::policy::{Band, InterestSplit, PolicyParams};

pub const CENTAVOS_PER_XLM_UNIT: i64 = 10_000_000; // stroops in 1 XLM

/// Seconds per schedule month. Calendar-exact due dates are a product nicety,
/// not a money invariant — a fixed 30-day month keeps every schedule
/// reproducible from (disbursed_at, installment) alone.
pub const MONTH_SECS: i64 = 30 * 24 * 3600;

/// The single rounding site (blueprint §1): banker's rounding, half-to-even.
/// `numer` may be negative; `denom` must be positive.
pub fn round_half_even(numer: i128, denom: i128) -> i64 {
    debug_assert!(denom > 0);
    let quot = numer.div_euclid(denom);
    let rem = numer.rem_euclid(denom); // 0..denom
    let twice = rem * 2;
    // Round up past half, and AT half only when the quotient is odd (to even).
    let round_up = twice > denom || (twice == denom && quot % 2 != 0);
    (if round_up { quot + 1 } else { quot }) as i64
}

/// The score band a borrower falls in, or None (score below every band =
/// not eligible to borrow yet).
pub fn band_for(score: i16, params: &PolicyParams) -> Option<&Band> {
    params
        .bands
        .iter()
        .find(|b| score >= b.min_score && score <= b.max_score)
}

/// Monthly price in basis points for a product, straight off the band.
pub fn rate_bps(product: &str, band: &Band) -> i32 {
    match product {
        "guarantor" => band.guarantor_bps,
        _ => band.secured_bps,
    }
}

/// The hard cap for a product: the band cap, doubled (per policy) when
/// guarantors carry the risk.
pub fn cap_for(product: &str, band: &Band, params: &PolicyParams) -> i64 {
    match product {
        "guarantor" => band.cap.saturating_mul(params.guarantor_cap_multiple.max(1)),
        _ => band.cap,
    }
}

/// Deposit-backed: borrowing `amount` requires freezing this much of the
/// borrower's own deposit (the >= amount/90% side of "borrow up to 90%").
/// Ceiling division — the collateral can round up a centavo, never down.
pub fn required_deposit_collateral(amount: i64, ltv_pct: i64) -> i64 {
    let numer = amount as i128 * 100;
    let denom = ltv_pct.max(1) as i128;
    ((numer + denom - 1) / denom) as i64
}

/// Deposit-backed: the most that can be borrowed against `available` centavos.
pub fn max_borrow_against_deposit(available: i64, ltv_pct: i64) -> i64 {
    (available as i128 * ltv_pct as i128 / 100) as i64
}

/// XLM: stroops that must be locked so the collateral is worth at least
/// `min_pct`% of the loan at the given rate. Ceiling at both steps — the
/// chain requirement can only ever round against the borrower, not the pool.
pub fn required_collateral_stroops(amount: i64, min_pct: i64, centavos_per_xlm: i64) -> i64 {
    let needed_centavos = {
        let numer = amount as i128 * min_pct as i128;
        (numer + 99) / 100
    };
    let numer = needed_centavos * CENTAVOS_PER_XLM_UNIT as i128;
    let denom = centavos_per_xlm.max(1) as i128;
    ((numer + denom - 1) / denom) as i64
}

/// What locked stroops are worth in centavos at the given rate (floor —
/// collateral is valued conservatively).
pub fn collateral_value_centavos(stroops: i64, centavos_per_xlm: i64) -> i64 {
    (stroops as i128 * centavos_per_xlm as i128 / CENTAVOS_PER_XLM_UNIT as i128) as i64
}

pub struct Installment {
    pub installment: i16,
    pub due_at: i64,
    pub principal_due: i64,
    pub interest_due: i64,
}

/// Reducing-balance monthly schedule (blueprint A3), computed once at
/// disbursement and pinned. Interest each month is `outstanding * bps/10000`
/// (banker's rounding); principal is equal slices with the final installment
/// absorbing the remainder, so `sum(principal_due) == principal` exactly.
pub fn build_schedule(principal: i64, rate_bps: i32, term_months: i16, start_at: i64) -> Vec<Installment> {
    let n = term_months as i64;
    let slice = principal / n;
    let mut outstanding = principal;
    let mut rows = Vec::with_capacity(term_months as usize);
    for i in 1..=term_months {
        let principal_due = if i == term_months {
            outstanding // final slice absorbs the rounding remainder
        } else {
            slice
        };
        let interest_due = round_half_even(outstanding as i128 * rate_bps as i128, 10_000);
        rows.push(Installment {
            installment: i,
            due_at: start_at + MONTH_SECS * i as i64,
            principal_due,
            interest_due,
        });
        outstanding -= principal_due;
    }
    rows
}

/// Where a peso of collected interest lands (D6, simplified while saver
/// payouts are a later slice): platform's share is rounded at the single
/// site, the reserve takes the deterministic remainder, so the split always
/// sums back to the collected centavo.
pub fn split_interest(interest: i64, split: &InterestSplit) -> (i64, i64) {
    let platform = round_half_even(interest as i128 * split.platform as i128, 100);
    let reserve = interest - platform;
    (platform, reserve)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::lending::policy::{InterestSplit, PolicyParams, TermRange};

    fn params() -> PolicyParams {
        PolicyParams {
            bands: vec![
                Band { min_score: 50, max_score: 69, cap: 500_000, secured_bps: 200, guarantor_bps: 600 },
                Band { min_score: 130, max_score: 150, cap: 10_000_000, secured_bps: 100, guarantor_bps: 300 },
            ],
            deposit_ltv_pct: 90,
            xlm_min_collateral_pct: 120,
            xlm_liquidation_pct: 110,
            guarantor_cap_multiple: 2,
            guarantors_max: 3,
            term_months: TermRange { min: 3, max: 12 },
            min_deposit: 10_000,
            min_loan: 50_000,
            interest_split: InterestSplit { savers: 0, platform: 80, reserve: 20 },
        }
    }

    use super::super::policy::Band;

    #[test]
    fn banker_rounding_is_half_to_even() {
        assert_eq!(round_half_even(25, 10), 2); // 2.5 -> 2
        assert_eq!(round_half_even(35, 10), 4); // 3.5 -> 4
        assert_eq!(round_half_even(26, 10), 3);
        assert_eq!(round_half_even(24, 10), 2);
    }

    #[test]
    fn bands_map_scores_to_caps_and_rates() {
        let p = params();
        assert!(band_for(49, &p).is_none());
        let b = band_for(50, &p).unwrap();
        assert_eq!((b.cap, b.secured_bps, b.guarantor_bps), (500_000, 200, 600));
        assert_eq!(cap_for("guarantor", b, &p), 1_000_000); // 2x with guarantors
        assert_eq!(cap_for("deposit_backed", b, &p), 500_000);
        assert_eq!(rate_bps("guarantor", b), 600);
        assert_eq!(rate_bps("xlm_collateral", b), 200);
    }

    #[test]
    fn deposit_ltv_round_trips_conservatively() {
        // Borrow 90% of what you must freeze — freezing then borrowing the
        // stated max never exceeds the LTV.
        for amount in [1, 99, 100, 4_999, 500_000, 123_457] {
            let req = required_deposit_collateral(amount, 90);
            assert!(max_borrow_against_deposit(req, 90) >= amount);
            assert!(max_borrow_against_deposit(req - 1, 90) < amount);
        }
    }

    #[test]
    fn xlm_collateral_requirement_covers_120_pct() {
        let rate = 1_800; // ₱18.00 / XLM
        let amount = 500_000; // ₱5,000 loan
        let stroops = required_collateral_stroops(amount, 120, rate);
        let value = collateral_value_centavos(stroops, rate);
        assert!(value >= amount * 120 / 100);
        // and not absurdly more than one stroop over
        assert!(collateral_value_centavos(stroops - 1, rate) < 600_000 + rate);
    }

    #[test]
    fn schedule_ties_to_the_centavo() {
        let principal = 1_000_000; // ₱10,000
        let rows = build_schedule(principal, 175, 7, 0);
        assert_eq!(rows.len(), 7);
        assert_eq!(rows.iter().map(|r| r.principal_due).sum::<i64>(), principal);
        // reducing balance: first month interest on full principal
        assert_eq!(rows[0].interest_due, round_half_even(principal as i128 * 175, 10_000));
        // interest strictly decreases as principal amortizes
        assert!(rows.windows(2).all(|w| w[0].interest_due >= w[1].interest_due));
        assert!(rows.last().unwrap().interest_due > 0);
    }

    #[test]
    fn interest_split_sums_back_exactly() {
        let split = InterestSplit { savers: 0, platform: 80, reserve: 20 };
        for interest in [0, 1, 99, 101, 12_345] {
            let (platform, reserve) = split_interest(interest, &split);
            assert_eq!(platform + reserve, interest);
        }
    }
}
