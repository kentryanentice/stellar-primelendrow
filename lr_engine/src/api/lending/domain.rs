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

// ---- the borrower's own share of a guarantor loan (SOW §4.1) --------------

/// Centavos of the principal the borrower must carry themselves before any
/// guarantor is asked for anything. Ceiling — the floor rounds towards the
/// borrower carrying more, never less.
pub fn required_borrower_cover(amount: i64, min_pct: i64) -> i64 {
    let numer = amount as i128 * min_pct.clamp(0, 100) as i128;
    ((numer + 99) / 100) as i64
}

/// How a guarantor loan is actually backed, once the borrower's two legs and
/// the guarantors' share are settled. Peso amounts throughout: the XLM leg is
/// stated as the centavos of principal it carries, and the stroops that must
/// be locked to carry it are `required_collateral_stroops(xlm, ratio, rate)` —
/// so the 120% over-collateralization applies to the borrower's XLM share
/// alone, exactly as it does on a pure collateral loan.
#[derive(Debug, PartialEq, Eq)]
pub struct CoverPlan {
    pub required: i64,
    pub deposit: i64,
    pub xlm: i64,
    pub total: i64,
    /// What the guarantors must pledge between them: the remainder.
    pub guarantor_gap: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CoverError {
    /// The two legs together fall short of the policy floor.
    BelowFloor { required: i64, offered: i64 },
    /// Negative legs, or a total past the whole principal.
    Invalid,
    /// Fully self-backed, so there is nothing for a guarantor to do.
    NoGapLeft,
}

/// PURE: settle the borrower's cover against the floor and work out what is
/// left for the guarantors. The caller supplies only INTENT — how much of the
/// cover each leg should carry — and every consequence is derived here.
pub fn plan_cover(
    amount: i64,
    deposit_cover: i64,
    xlm_cover: i64,
    min_pct: i64,
) -> Result<CoverPlan, CoverError> {
    if deposit_cover < 0 || xlm_cover < 0 {
        return Err(CoverError::Invalid);
    }
    let total = (deposit_cover as i128 + xlm_cover as i128)
        .try_into()
        .map_err(|_| CoverError::Invalid)?;
    if total > amount {
        return Err(CoverError::Invalid);
    }
    let required = required_borrower_cover(amount, min_pct);
    if total < required {
        return Err(CoverError::BelowFloor { required, offered: total });
    }
    let guarantor_gap = amount - total;
    if guarantor_gap <= 0 {
        return Err(CoverError::NoGapLeft);
    }
    Ok(CoverPlan { required, deposit: deposit_cover, xlm: xlm_cover, total, guarantor_gap })
}

// ---- pricing the collateral (SOW §3.10, "Price source and oracle bounds") --
//
// The feeds themselves are I/O and live in `infra::oracle`; deciding ONE
// number out of what they said is arithmetic, so it lives here with the rest
// of the money rules and is tested with plain integers.

/// Fixed-point scale shared with `infra::oracle`: rates are integers of
/// hundred-millionths, never floats.
pub const RATE_SCALE: i64 = 100_000_000;

/// XLM/USD times USD/PHP -> PHP per XLM, all three scaled by RATE_SCALE.
/// This is the derived rate the SOW requires be recorded with the loan when
/// no direct XLM/PHP feed answers.
pub fn derive_php_per_xlm(usd_per_xlm: i64, php_per_usd: i64) -> i64 {
    (usd_per_xlm as i128 * php_per_usd as i128 / RATE_SCALE as i128) as i64
}

/// A scaled PHP-per-XLM rate -> the whole centavos the collateral rules
/// actually value against, at the single rounding site.
pub fn scaled_to_centavos(php_per_xlm: i64) -> i64 {
    round_half_even(php_per_xlm as i128 * 100, RATE_SCALE as i128)
}

/// Middle value of a non-empty set; an even count averages the two middles
/// at the single rounding site. A median (not a mean) is the point: one
/// provider printing a wild number moves it by nothing.
pub fn median(values: &[i64]) -> Option<i64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let mid = sorted.len() / 2;
    Some(if sorted.len() % 2 == 1 {
        sorted[mid]
    } else {
        round_half_even(sorted[mid - 1] as i128 + sorted[mid] as i128, 2)
    })
}

/// The agreed rate, or None — which is a refusal to price, not a fallback.
///
/// Take the median, drop every quote further than `max_deviation_bps` from
/// it, and re-median the survivors. `min_sources` independent feeds must
/// still stand after the drop: one feed alone can be wrong in a way nothing
/// else contradicts, so it is never enough to lend against.
pub fn agree_on_rate(quotes: &[i64], min_sources: usize, max_deviation_bps: i64) -> Option<i64> {
    let min_sources = min_sources.max(1);
    if quotes.len() < min_sources {
        return None;
    }
    let first = median(quotes)?;
    let kept: Vec<i64> = quotes
        .iter()
        .copied()
        .filter(|q| {
            let drift = (*q as i128 - first as i128).abs();
            drift * 10_000 <= first as i128 * max_deviation_bps as i128
        })
        .collect();
    if kept.len() < min_sources {
        return None;
    }
    median(&kept)
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
            guarantors_max: 2,
            borrower_cover_min_pct: 50,
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
    fn borrower_cover_floor_rounds_towards_the_borrower() {
        // Half of ₱10,000 is exact.
        assert_eq!(required_borrower_cover(1_000_000, 50), 500_000);
        // An odd centavo rounds UP, so the floor is never undershot.
        assert_eq!(required_borrower_cover(1_000_001, 50), 500_001);
        assert_eq!(required_borrower_cover(333, 50), 167);
        // The parameter is honoured, not assumed to be 50.
        assert_eq!(required_borrower_cover(1_000_000, 60), 600_000);
    }

    #[test]
    fn cover_can_be_deposit_xlm_or_a_mix_of_both() {
        let amount = 1_000_000; // ₱10,000, so the floor is ₱5,000
        // All deposit.
        let all_deposit = plan_cover(amount, 500_000, 0, 50).unwrap();
        assert_eq!(all_deposit.guarantor_gap, 500_000);
        // All XLM.
        let all_xlm = plan_cover(amount, 0, 500_000, 50).unwrap();
        assert_eq!(all_xlm.guarantor_gap, 500_000);
        // A mix that meets the floor exactly.
        let mixed = plan_cover(amount, 200_000, 300_000, 50).unwrap();
        assert_eq!(mixed.total, 500_000);
        assert_eq!(mixed.guarantor_gap, 500_000);
        // Covering MORE than the floor shrinks the guarantors' share.
        let generous = plan_cover(amount, 400_000, 300_000, 50).unwrap();
        assert_eq!(generous.guarantor_gap, 300_000);
    }

    #[test]
    fn cover_below_the_floor_is_refused() {
        let amount = 1_000_000;
        // One centavo short, from either leg or both.
        assert_eq!(
            plan_cover(amount, 499_999, 0, 50),
            Err(CoverError::BelowFloor { required: 500_000, offered: 499_999 })
        );
        assert_eq!(
            plan_cover(amount, 0, 499_999, 50),
            Err(CoverError::BelowFloor { required: 500_000, offered: 499_999 })
        );
        assert_eq!(
            plan_cover(amount, 250_000, 249_999, 50),
            Err(CoverError::BelowFloor { required: 500_000, offered: 499_999 })
        );
        // Nothing offered at all is the same refusal, not a special case.
        assert!(matches!(plan_cover(amount, 0, 0, 50), Err(CoverError::BelowFloor { .. })));
    }

    #[test]
    fn cover_cannot_exceed_the_loan_or_leave_no_gap() {
        let amount = 1_000_000;
        // Fully self-backed: this is a deposit or collateral loan, not a
        // guarantor one, and is refused rather than issued with no guarantors.
        assert_eq!(plan_cover(amount, 1_000_000, 0, 50), Err(CoverError::NoGapLeft));
        // Past the principal entirely.
        assert_eq!(plan_cover(amount, 900_000, 200_000, 50), Err(CoverError::Invalid));
        // Negative legs never reach the arithmetic.
        assert_eq!(plan_cover(amount, -1, 600_000, 50), Err(CoverError::Invalid));
        assert_eq!(plan_cover(amount, 600_000, -1, 50), Err(CoverError::Invalid));
        // And a leg large enough to overflow the sum is refused, not wrapped.
        assert_eq!(plan_cover(amount, i64::MAX, i64::MAX, 50), Err(CoverError::Invalid));
    }

    #[test]
    fn the_xlm_leg_is_over_collateralized_at_the_policy_ratio() {
        // The borrower carries ₱5,000 of a ₱10,000 loan on XLM alone. The
        // coins locked must be worth 120% of THAT LEG — ₱6,000 — not 120% of
        // the whole loan, and not merely the leg itself.
        let rate = 1_800; // ₱18.00 / XLM
        let plan = plan_cover(1_000_000, 0, 500_000, 50).unwrap();
        let stroops = required_collateral_stroops(plan.xlm, 120, rate);
        let value = collateral_value_centavos(stroops, rate);
        assert!(value >= 600_000);
        assert!(value < 600_000 + rate); // and not materially more
    }

    #[test]
    fn derives_the_php_rate_through_the_dollar() {
        // $0.39 per XLM at ₱58.50 per $ = ₱22.815 per XLM -> ₱22.82 (half-even
        // lands on the even centavo at exactly half; here it rounds up).
        let usd_per_xlm = 39 * RATE_SCALE / 100;
        let php_per_usd = 585 * RATE_SCALE / 10;
        let scaled = derive_php_per_xlm(usd_per_xlm, php_per_usd);
        assert_eq!(scaled, 2_281_500_000);
        assert_eq!(scaled_to_centavos(scaled), 2282);
        // and the identity leg: anything times ₱1.00/$ is itself
        assert_eq!(derive_php_per_xlm(usd_per_xlm, RATE_SCALE), usd_per_xlm);
    }

    #[test]
    fn median_is_the_middle_not_the_mean() {
        assert_eq!(median(&[]), None);
        assert_eq!(median(&[7]), Some(7));
        assert_eq!(median(&[9, 1, 5]), Some(5));
        assert_eq!(median(&[4, 2]), Some(3));
        // one absurd outlier cannot drag the middle
        assert_eq!(median(&[2280, 2282, 2284, 1]), Some(2281));
    }

    #[test]
    fn agreement_needs_a_quorum_that_survives_the_deviation_band() {
        let band = 500; // 5%
        // three feeds within a whisker of each other -> their middle
        assert_eq!(agree_on_rate(&[2280, 2282, 2284], 2, band), Some(2282));
        // one feed is never enough, however plausible
        assert_eq!(agree_on_rate(&[2282], 2, band), None);
        // an out-of-band quote is dropped and the rest still agree
        assert_eq!(agree_on_rate(&[2280, 2282, 9000], 2, band), Some(2281));
        // two feeds that disagree wildly leave no quorum -> refuse to price
        assert_eq!(agree_on_rate(&[1000, 9000], 2, band), None);
        // exactly at the band edge is kept (2282 * 5% = 114.1)
        assert_eq!(agree_on_rate(&[2282, 2282, 2396], 3, band), Some(2282));
        assert_eq!(agree_on_rate(&[2282, 2282, 2397], 3, band), None);
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
