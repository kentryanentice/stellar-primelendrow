//! PURE lending rules — no DB, no clock, no HTTP. Every function here is a
//! plain function of integers and the rulebook, unit-tested with plain
//! numbers at the bottom of the file. If it can't be tested that way, it
//! does not belong in this file (blueprint §3 dependency rule).
//!
//! Money is whole centavos in i64; intermediate products use i128 so a cap
//! times a percentage can never overflow on the way to a valid result.

use super::policy::{Band, DepositLimitTier, GuarantorTier, InterestSplit, PolicyParams, RailFees};

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

/// Splits `total` across `weights` in proportion to each weight, exact to the
/// centavo.
///
/// This is how a guarantor claim is shared (SOW deliverable 2: "a claim is
/// shared between guarantors in proportion to what each pledged"). Seizure used
/// to consume pledged lots in order of the depositor's age, which meant that on
/// a partial claim the guarantor who happened to deposit into the pool earlier
/// paid the whole thing and the other paid nothing — a tiebreaker with no
/// relationship to the loan, the pledge, or anything either of them agreed to.
///
/// **Rounding, at one site.** Each share is `total * weight / sum`, rounded
/// half-to-even like every other money split in this module. Rounding
/// independently leaves a residual of a few centavos that belongs to nobody, so
/// the shares are summed and the difference handed to the **largest** weight —
/// the same "final recipient absorbs the remainder" pattern
/// `split_interest_parts` uses. Largest rather than first: the residual is at most a centavo per
/// participant, and putting it on the biggest pledge is the least surprising
/// place for it. The result therefore sums to `total` exactly, always.
///
/// Ties on "largest" go to the earliest of the tied weights, so the answer is
/// deterministic for a given input order — two runs over the same loan produce
/// the same split.
///
/// Returns one share per weight, in the same order. A zero or negative `total`
/// yields all zeros, and weights summing to zero do too: there is nothing to
/// share and nobody to share it by.
pub fn apportion(total: i64, weights: &[i64]) -> Vec<i64> {
    let sum: i128 = weights.iter().map(|w| (*w).max(0) as i128).sum();
    if total <= 0 || sum == 0 || weights.is_empty() {
        return vec![0; weights.len()];
    }

    let mut shares: Vec<i64> = weights
        .iter()
        .map(|w| round_half_even(total as i128 * (*w).max(0) as i128, sum))
        .collect();

    // The residual: what rounding gained or lost against the total.
    let allocated: i64 = shares.iter().sum();
    let residual = total - allocated;
    if residual != 0 {
        // Strict `>` keeps the EARLIEST of any tied weights, which is what
        // makes the split deterministic for a given input order.
        let mut largest = 0usize;
        for (i, w) in weights.iter().enumerate() {
            if *w > weights[largest] {
                largest = i;
            }
        }
        shares[largest] += residual;
    }

    shares
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

/// The rulebook's interest split is internally consistent (SOW deliverable 3):
/// the four fixed shares are non-negative and sum to exactly 100, the
/// guarantor cap fits inside the risk band, and the tier table is contiguous,
/// ascending, never above the cap, never decreasing as score rises, and covers
/// every score a pricing band can produce. Checked when the rulebook loads, so
/// a bad policy change fails loudly instead of mis-splitting money.
pub fn check_interest_split(params: &PolicyParams) -> Result<(), &'static str> {
    let s = &params.interest_split;
    let fixed = [s.platform, s.reserve, s.depositors, s.risk_band];
    if fixed.iter().any(|p| *p < 0) {
        return Err("negative share");
    }
    if fixed.iter().sum::<i64>() != 100 {
        return Err("platform + reserve + depositors + risk_band must equal 100");
    }
    if s.guarantor_cap < 0 || s.guarantor_cap > s.risk_band {
        return Err("guarantor_cap must be within the risk band");
    }

    let tiers = &s.guarantor_tiers;
    let (Some(first), Some(last)) = (tiers.first(), tiers.last()) else {
        return Err("no guarantor tiers");
    };
    for t in tiers {
        if t.min_score > t.max_score {
            return Err("tier min_score above max_score");
        }
        if t.share < 0 || t.share > s.guarantor_cap {
            return Err("tier share outside 0..=guarantor_cap");
        }
    }
    for w in tiers.windows(2) {
        if w[1].min_score != w[0].max_score + 1 {
            return Err("tiers must be contiguous and ascending");
        }
        if w[1].share < w[0].share {
            return Err("tier share must not fall as score rises");
        }
    }
    let band_min = params.bands.iter().map(|b| b.min_score).min();
    let band_max = params.bands.iter().map(|b| b.max_score).max();
    if band_min.is_some_and(|m| m < first.min_score) || band_max.is_some_and(|m| m > last.max_score) {
        return Err("tiers do not cover every banded score");
    }
    Ok(())
}

/// Splits `total` across `caps` in proportion to each cap, exact to the
/// centavo, and never gives anyone more than their own cap.
///
/// This is how a loan is funded from, and paid back to, many members at once:
/// each member's slice is proportional to their balance, but a slice larger
/// than the balance itself would lock (or release) money that isn't there.
/// `apportion`'s half-even rounding plus "residual to the largest" can do
/// exactly that when the total is close to the sum of the caps, so this uses
/// largest-remainder rounding instead: everyone gets the floor of their exact
/// share, then the few centavos left over go one each to the largest
/// fractional remainders. A member only receives that extra centavo when their
/// exact share was a fraction below their cap, so floor + 1 can never pass it.
///
/// `total` above the sum of the caps is clamped to it (there is only so much
/// to take). Ties on the remainder go to the earliest index, so the result is
/// deterministic for a given input order.
pub fn apportion_capped(total: i64, caps: &[i64]) -> Vec<i64> {
    let caps: Vec<i128> = caps.iter().map(|c| (*c).max(0) as i128).collect();
    let sum: i128 = caps.iter().sum();
    let total = (total.max(0) as i128).min(sum);
    if total == 0 {
        return vec![0; caps.len()];
    }

    let mut shares: Vec<i64> = caps.iter().map(|c| (total * c / sum) as i64).collect();
    let mut left = (total - shares.iter().map(|s| *s as i128).sum::<i128>()) as usize;
    let mut by_remainder: Vec<usize> = (0..caps.len()).filter(|i| total * caps[*i] % sum != 0).collect();
    by_remainder.sort_by(|a, b| (total * caps[*b] % sum).cmp(&(total * caps[*a] % sum)).then(a.cmp(b)));
    for i in by_remainder {
        if left == 0 {
            break;
        }
        shares[i] += 1;
        left -= 1;
    }
    shares
}

/// How much of a loan's funding to unlock when `principal_paid` of it comes
/// back: the same fraction of what is still locked as the payment is of what
/// was still owed, so members' money stays at risk exactly as long as the
/// loan's balance does. The final payment unlocks everything, which is also
/// what sweeps up any centavo the proportional rounding left behind.
pub fn funding_to_release(locked: i64, principal_paid: i64, outstanding_before: i64) -> i64 {
    if locked <= 0 || principal_paid <= 0 {
        return 0;
    }
    if principal_paid >= outstanding_before {
        return locked;
    }
    round_half_even(locked as i128 * principal_paid as i128, outstanding_before as i128).clamp(0, locked)
}

/// What the pool's members must fund when a loan disburses: the principal
/// less whatever the borrower backs with their own locked deposit. A
/// deposit-backed loan locks at least principal / LTV of the borrower's own
/// money, so this is 0 and nobody else's balance is touched.
pub fn pool_funded_amount(principal: i64, own_deposit_backing: i64) -> i64 {
    (principal - own_deposit_backing.max(0)).max(0)
}

// ===========================================================================
// Exact repayments
// ===========================================================================

/// The one amount a repayment may be: everything still owed on the earliest
/// installment that isn't fully paid — its outstanding interest plus its
/// outstanding principal — with that installment's number. `None` when every
/// installment is settled.
///
/// `rows` are `(installment, interest_due, interest_paid, principal_due,
/// principal_paid)` in installment order. A member pays exactly this, never
/// more (no overpayment turned into a deposit) and never less (no partial
/// installment), which is what lets the engine set the amount on the payment
/// page itself rather than accept one.
pub fn next_installment_due(rows: &[(i16, i64, i64, i64, i64)]) -> Option<(i16, i64)> {
    rows.iter().find_map(|(installment, interest_due, interest_paid, principal_due, principal_paid)| {
        let owed = (interest_due - interest_paid).max(0) + (principal_due - principal_paid).max(0);
        (owed > 0).then_some((*installment, owed))
    })
}

// ===========================================================================
// Payment provider fees
// ===========================================================================

/// The fee table is usable: rates in 0–99.99%, fixed fees and caps not
/// negative. A receive rate of 100% or more would make grossing a repayment up
/// impossible.
pub fn check_payment_fees(params: &PolicyParams) -> Result<(), &'static str> {
    for fees in [&params.payment_fees.paypal, &params.payment_fees.stripe] {
        if !(0..10_000).contains(&fees.receive_bps) || !(0..10_000).contains(&fees.payout_bps) {
            return Err("fee rates must be 0..10000 bps");
        }
        if fees.receive_fixed < 0 || fees.payout_cap < 0 {
            return Err("fixed fees and caps must not be negative");
        }
    }
    Ok(())
}

/// What the provider is expected to keep when `charged` centavos are paid in:
/// the rate, rounded half-even, plus the fixed fee. An estimate — the
/// provider's own reported fee always wins once the payment is captured.
pub fn receive_fee_estimate(charged: i64, fees: &RailFees) -> i64 {
    round_half_even(charged.max(0) as i128 * fees.receive_bps as i128, 10_000) + fees.receive_fixed
}

/// The total to charge so that, after the expected receiving fee, exactly
/// `applies` centavos are left: the smallest total whose estimated fee leaves
/// at least `applies`. Returns `(total, fee)`, `total = applies + fee`.
///
/// This is how a repayment passes the fee to the borrower while the loan still
/// receives exactly its installment.
pub fn gross_up(applies: i64, fees: &RailFees) -> (i64, i64) {
    let applies = applies.max(0);
    // Closed form first: total = (applies + fixed) / (1 - rate), rounded up...
    let denom = (10_000 - fees.receive_bps) as i128;
    let mut total = (((applies + fees.receive_fixed) as i128 * 10_000 + denom - 1) / denom) as i64;
    // ...then nudge for the half-even rounding of the fee, so the guarantee
    // holds exactly rather than approximately.
    while total > applies && total - 1 - receive_fee_estimate(total - 1, fees) >= applies {
        total -= 1;
    }
    while total - receive_fee_estimate(total, fees) < applies {
        total += 1;
    }
    (total, total - applies)
}

/// What the provider charges the sender for a payout that SENDS `sent`: the
/// rate on the amount sent, rounded half-even, capped. PayPal takes this on top
/// of the transfer, out of the business balance.
pub fn payout_charge(sent: i64, fees: &RailFees) -> i64 {
    let fee = round_half_even(sent.max(0) as i128 * fees.payout_bps as i128, 10_000);
    if fees.payout_cap > 0 { fee.min(fees.payout_cap) } else { fee }
}

/// The fee deducted from a payout claim of `amount`, so that what is sent plus
/// what the provider charges for sending it fits inside the claim: the largest
/// `sent` with `sent + payout_charge(sent) <= amount`, and the fee is the rest.
/// The pool therefore pays out exactly the member's claim.
///
/// Taking the rate from the claim instead undercounts, because PayPal charges
/// its percentage on what it SENDS, on top: a ₱20,000 withdrawal that sent
/// ₱19,950 cost the business ₱399 more, not ₱50. Never the whole amount — at
/// least a centavo is always sent.
pub fn payout_fee(amount: i64, fees: &RailFees) -> i64 {
    let amount = amount.max(0);
    if amount <= 1 {
        return 0;
    }
    // Start near the answer — uncapped, sent = amount / (1 + rate); with a cap,
    // sent is at least amount - cap — then settle the rounding exactly.
    let mut sent = (amount as i128 * 10_000 / (10_000 + fees.payout_bps as i128)) as i64;
    if fees.payout_cap > 0 {
        sent = sent.max(amount - fees.payout_cap);
    }
    sent = sent.clamp(1, amount);
    while sent > 1 && sent + payout_charge(sent, fees) > amount {
        sent -= 1;
    }
    while sent < amount && sent + 1 + payout_charge(sent + 1, fees) <= amount {
        sent += 1;
    }
    amount - sent
}

// ===========================================================================
// AML deposit limits
// ===========================================================================

/// A member's effective deposit limits, in centavos.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct DepositLimitsFor {
    pub per_deposit: i64,
    pub daily: i64,
    pub monthly: i64,
    pub max_balance: i64,
}

/// The deposit-limit table is internally consistent: at least one tier,
/// contiguous and ascending by score up to the top of the 0–150 range, every
/// limit positive, `per_deposit <= daily <= monthly` and `per_deposit <=
/// max_balance` within a tier, no limit falling as score rises, and a
/// below-floor percent in 1..=100. Checked when the rulebook loads.
pub fn check_deposit_limits(params: &PolicyParams) -> Result<(), &'static str> {
    let limits = &params.deposit_limits;
    let (Some(first), Some(last)) = (limits.tiers.first(), limits.tiers.last()) else {
        return Err("no deposit limit tiers");
    };
    if !(1..=100).contains(&limits.below_floor_pct) {
        return Err("below_floor_pct must be 1..=100");
    }
    if first.min_score < 0 || last.max_score < 150 {
        return Err("deposit tiers must reach score 150");
    }
    for t in &limits.tiers {
        if t.min_score > t.max_score {
            return Err("deposit tier min_score above max_score");
        }
        if t.per_deposit <= 0 || t.daily <= 0 || t.monthly <= 0 || t.max_balance <= 0 {
            return Err("deposit limits must be positive");
        }
        if t.per_deposit > t.daily || t.daily > t.monthly || t.per_deposit > t.max_balance {
            return Err("deposit limits must satisfy per_deposit <= daily <= monthly and per_deposit <= max_balance");
        }
    }
    for w in limits.tiers.windows(2) {
        if w[1].min_score != w[0].max_score + 1 {
            return Err("deposit tiers must be contiguous and ascending");
        }
        let (a, b) = (&w[0], &w[1]);
        if b.per_deposit < a.per_deposit || b.daily < a.daily || b.monthly < a.monthly || b.max_balance < a.max_balance {
            return Err("deposit limits must not fall as score rises");
        }
    }
    Ok(())
}

/// The limits for a score. Inside a tier: that tier. Below the lowest tier
/// (a member whose score has dropped under 50): `below_floor_pct` of the
/// lowest tier's limits, rounded down. Above the highest: the highest.
pub fn deposit_limits_for(score: i16, params: &PolicyParams) -> DepositLimitsFor {
    let limits = &params.deposit_limits;
    let of = |t: &DepositLimitTier| DepositLimitsFor {
        per_deposit: t.per_deposit,
        daily: t.daily,
        monthly: t.monthly,
        max_balance: t.max_balance,
    };
    if let Some(t) = limits.tiers.iter().find(|t| score >= t.min_score && score <= t.max_score) {
        return of(t);
    }
    match (limits.tiers.first(), limits.tiers.last()) {
        (Some(first), _) if score < first.min_score => {
            let scale = |v: i64| (v as i128 * limits.below_floor_pct as i128 / 100) as i64;
            DepositLimitsFor {
                per_deposit: scale(first.per_deposit),
                daily: scale(first.daily),
                monthly: scale(first.monthly),
                max_balance: scale(first.max_balance),
            }
        }
        (_, Some(last)) => of(last),
        // `check_deposit_limits` refuses an empty table when the rulebook
        // loads, so this is unreachable; zero means "nothing allowed".
        _ => DepositLimitsFor { per_deposit: 0, daily: 0, monthly: 0, max_balance: 0 },
    }
}

/// The largest deposit a member may start right now: the tightest of their
/// per-deposit limit, what is left of the 24-hour and 30-day limits, and what
/// is left under the balance cap. `used_*` and `balance` already include
/// deposits the member has started and not yet finished, so two checkouts
/// opened at once can't both fit under the same limit. Never negative.
pub fn deposit_allowance(limits: DepositLimitsFor, used_24h: i64, used_30d: i64, balance: i64) -> i64 {
    limits
        .per_deposit
        .min(limits.daily - used_24h)
        .min(limits.monthly - used_30d)
        .min(limits.max_balance - balance)
        .max(0)
}

/// The guarantor tier a score falls in — the split's counterpart to `band_for`.
pub fn guarantor_tier_for(score: i16, split: &InterestSplit) -> Option<&GuarantorTier> {
    split
        .guarantor_tiers
        .iter()
        .find(|t| score >= t.min_score && score <= t.max_score)
}

/// Where one interest payment lands, in whole centavos. Always sums to the
/// interest it was split from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct InterestParts {
    pub platform: i64,
    pub reserve: i64,
    pub depositors: i64,
    pub guarantor: i64,
    pub recovery_fund: i64,
}

/// The four-way split (SOW deliverable 3). The three fixed shares are each
/// rounded at the single site; the risk band is the deterministic remainder
/// of the interest, so the parts always sum back exactly. The guarantor's
/// share (percent of the whole interest, `None` when nobody guarantees) is
/// rounded the same way and capped at the band, and the recovery fund keeps
/// the rest of the band — so it can be squeezed to zero but never negative.
///
/// This is the single-guarantor form the published worked example uses.
/// Repayments go through `book_interest`, which handles any number of
/// guarantors at different tiers and reduces to this for one.
pub fn split_interest_parts(interest: i64, split: &InterestSplit, guarantor_share: Option<i64>) -> InterestParts {
    let pct = |p: i64| round_half_even(interest as i128 * p as i128, 100);
    let platform = pct(split.platform);
    let reserve = pct(split.reserve);
    let depositors = pct(split.depositors);
    let band = interest - platform - reserve - depositors;
    let guarantor = guarantor_share.map_or(0, |s| pct(s.min(split.guarantor_cap)).clamp(0, band.max(0)));
    InterestParts { platform, reserve, depositors, guarantor, recovery_fund: band - guarantor }
}

/// One accepted guarantor on the loan being repaid.
#[derive(Clone, Copy, Debug)]
pub struct GuarantorStake {
    pub pledge: i64,
    /// The guarantor's OWN score when the payment lands — reputation is the
    /// guarantor's, and a guarantor who loses score on a claim drops a tier.
    pub score: i16,
}

/// One repayment's interest, as the books take it.
#[derive(Debug, PartialEq, Eq)]
pub struct BookedInterest {
    pub parts: InterestParts,
    /// Each depositor's slice of `parts.depositors`, in `balances` order.
    pub to_depositors: Vec<i64>,
    /// Each guarantor's slice of `parts.guarantor`, in `guarantors` order.
    pub to_guarantors: Vec<i64>,
    /// The tier percent each guarantor was paid at, in `guarantors` order
    /// (0 for a score below every tier).
    pub guarantor_tiers: Vec<i64>,
}

/// Everything a repayment decides about its interest, in one pure step.
///
/// 1. **Fixed shares.** Platform, reserve and depositors are each rounded once;
///    the risk band is what remains of the interest.
/// 2. **Guarantors.** Each guarantor earns their own tier's percent on their
///    share of the pledges, so the guarantors' combined percent is the
///    pledge-weighted average of their tiers — one guarantor backing the whole
///    gap at 10% earns 10% of the interest, exactly the published example; two
///    at 10% and 25% with equal pledges earn 17.5% between them. That combined
///    amount is rounded ONCE (rounding each guarantor separately would leak a
///    centavo per guarantor), capped at the band, then apportioned between
///    them by `tier × pledge`. The recovery fund keeps the rest of the band.
/// 3. **Depositors.** The depositors' share goes to every member in proportion
///    to `balances` (each member's whole deposit balance when the payment
///    lands). With no balance anywhere there is nobody to pay, so the share
///    stays in the reserve and is recorded there.
///
/// Invariants, all tested: `parts` sums to `interest`; `to_depositors` sums to
/// `parts.depositors`; `to_guarantors` sums to `parts.guarantor`; nothing is
/// negative. Those three sums are what keep the repayment's postings balanced
/// and every member's new lot equal to the member_deposits credit.
pub fn book_interest(
    interest: i64,
    split: &InterestSplit,
    balances: &[i64],
    guarantors: &[GuarantorStake],
) -> BookedInterest {
    let interest = interest.max(0);
    let pct = |p: i64| round_half_even(interest as i128 * p as i128, 100);
    let platform = pct(split.platform);
    let mut reserve = pct(split.reserve);
    let mut depositors = pct(split.depositors);
    let band = (interest - platform - reserve - depositors).max(0);

    let guarantor_tiers: Vec<i64> = guarantors
        .iter()
        .map(|g| guarantor_tier_for(g.score, split).map_or(0, |t| t.share.clamp(0, split.guarantor_cap)))
        .collect();
    let weights: Vec<i64> = guarantors
        .iter()
        .zip(&guarantor_tiers)
        .map(|(g, tier)| g.pledge.max(0).saturating_mul(*tier))
        .collect();
    let total_pledged: i128 = guarantors.iter().map(|g| g.pledge.max(0) as i128).sum();
    let guarantor = if total_pledged == 0 {
        0
    } else {
        let weighted: i128 = weights.iter().map(|w| *w as i128).sum();
        round_half_even(interest as i128 * weighted, total_pledged * 100).clamp(0, band)
    };
    let to_guarantors = apportion(guarantor, &weights);
    // `apportion` returns zeros when every weight is zero, and `guarantor` is
    // zero then too, so the two always agree.
    debug_assert_eq!(to_guarantors.iter().sum::<i64>(), guarantor);

    let mut to_depositors = apportion(depositors, balances);
    if to_depositors.iter().sum::<i64>() != depositors {
        // Nobody holds a balance to share by.
        reserve += depositors;
        depositors = 0;
        to_depositors = vec![0; balances.len()];
    }

    BookedInterest {
        parts: InterestParts { platform, reserve, depositors, guarantor, recovery_fund: band - guarantor },
        to_depositors,
        to_guarantors,
        guarantor_tiers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::lending::policy::{InterestSplit, PolicyParams, TermRange};

    /// The SOW's published split and tier table, verbatim.
    fn sow_split() -> InterestSplit {
        let tier = |min_score, max_score, share| GuarantorTier { min_score, max_score, share };
        InterestSplit {
            platform: 10,
            reserve: 20,
            depositors: 40,
            risk_band: 30,
            guarantor_cap: 25,
            guarantor_tiers: vec![
                tier(50, 79, 10),
                tier(80, 104, 15),
                tier(105, 129, 20),
                tier(130, 150, 25),
            ],
        }
    }

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
            interest_split: sow_split(),
            deposit_limits: sow_deposit_limits(),
            payment_fees: crate::api::lending::policy::PaymentFees { paypal: paypal_ph(), stripe: paypal_ph() },
        }
    }

    /// PayPal Philippines' published rates: 3.40% + ₱15 in, 2% capped at ₱50 out.
    fn paypal_ph() -> RailFees {
        RailFees { receive_bps: 340, receive_fixed: 1_500, payout_bps: 200, payout_cap: 5_000 }
    }

    fn sow_deposit_limits() -> crate::api::lending::policy::DepositLimits {
        let tier = |min_score, max_score, per_deposit, daily, monthly, max_balance| DepositLimitTier {
            min_score, max_score, per_deposit, daily, monthly, max_balance,
        };
        crate::api::lending::policy::DepositLimits {
            tiers: vec![
                tier(0, 69, 2_000_000, 5_000_000, 10_000_000, 20_000_000),
                tier(70, 150, 5_000_000, 10_000_000, 25_000_000, 50_000_000),
            ],
            below_floor_pct: 50,
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
    fn a_claim_is_shared_in_proportion_to_what_each_pledged() {
        // The case the sprint plan names: a partial claim across two unequal
        // pledges. Age-ordered seizure gave one guarantor the whole ₱2,000 and
        // the other nothing; proportion gives 3:2.
        assert_eq!(apportion(200_000, &[300_000, 200_000]), vec![120_000, 80_000]);

        // Equal pledges split equally.
        assert_eq!(apportion(100_000, &[250_000, 250_000]), vec![50_000, 50_000]);

        // A claim that exhausts everything still lands on the pledges exactly.
        assert_eq!(apportion(500_000, &[300_000, 200_000]), vec![300_000, 200_000]);

        // One guarantor.
        assert_eq!(apportion(123_456, &[500_000]), vec![123_456]);
    }

    #[test]
    fn apportioned_shares_always_sum_to_the_total() {
        // Nothing divides evenly here — the residual has to go somewhere, and
        // "somewhere" must never be "lost".
        for total in [1i64, 2, 7, 99, 101, 1_000, 33_333, 1_000_001] {
            for weights in [
                vec![300_000i64, 400_000],
                vec![1, 1, 1],
                vec![7, 11, 13],
                vec![999_999, 1],
                vec![100_000, 100_000, 100_000],
            ] {
                let shares = apportion(total, &weights);
                assert_eq!(
                    shares.iter().sum::<i64>(),
                    total,
                    "total {total} across {weights:?} lost or invented centavos"
                );
                // Nobody is charged a negative amount.
                assert!(shares.iter().all(|s| *s >= 0), "negative share for {weights:?}");
            }
        }
    }

    #[test]
    fn the_residual_lands_on_the_largest_pledge() {
        // ₱10.00 across 1:1:1 is ₱3.33 each with a centavo left over. The
        // weights are equal, so the earliest wins the tie and the result is
        // still deterministic.
        assert_eq!(apportion(1000, &[100, 100, 100]), vec![334, 333, 333]);

        // With an unambiguous largest, the residual goes there rather than to
        // whoever happens to be first.
        let shares = apportion(1000, &[100, 900]);
        assert_eq!(shares.iter().sum::<i64>(), 1000);
        assert_eq!(shares, vec![100, 900]);
    }

    #[test]
    fn nothing_to_share_or_nobody_to_share_by() {
        assert_eq!(apportion(0, &[100, 200]), vec![0, 0]);
        assert_eq!(apportion(-5, &[100, 200]), vec![0, 0]);
        // Pledges that sum to nothing can't absorb a claim — the caller falls
        // through to the reserve fund rather than dividing by zero.
        assert_eq!(apportion(1000, &[0, 0]), vec![0, 0]);
        assert!(apportion(1000, &[]).is_empty());
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
    fn the_published_rulebook_passes_its_own_checks() {
        assert_eq!(check_interest_split(&params()), Ok(()));
    }

    #[test]
    fn a_malformed_split_is_refused() {
        let broken = |f: fn(&mut InterestSplit)| {
            let mut p = params();
            f(&mut p.interest_split);
            check_interest_split(&p)
        };
        // Fixed shares that don't sum to 100 invent or lose centavos.
        assert!(broken(|s| s.depositors = 39).is_err());
        assert!(broken(|s| { s.platform = -10; s.depositors = 60 }).is_err());
        // A cap past the band would push the recovery fund negative.
        assert!(broken(|s| s.guarantor_cap = 31).is_err());
        // A tier above the cap.
        assert!(broken(|s| s.guarantor_tiers[3].share = 26).is_err());
        // A gap between tiers leaves some score with no share.
        assert!(broken(|s| s.guarantor_tiers[1].min_score = 81).is_err());
        // Better reputation never earns less.
        assert!(broken(|s| s.guarantor_tiers[2].share = 12).is_err());
        // Tiers that stop short of the top band.
        assert!(broken(|s| s.guarantor_tiers[3].max_score = 149).is_err());
        assert!(broken(|s| s.guarantor_tiers.clear()).is_err());
    }

    #[test]
    fn the_sow_worked_example_to_the_centavo() {
        // ₱1,000 at 2%/mo — the first payment's interest is ₱20.00.
        let s = sow_split();
        let parts = |g| split_interest_parts(2_000, &s, g);
        let row = |guarantor, recovery_fund| InterestParts {
            platform: 200, reserve: 400, depositors: 800, guarantor, recovery_fund,
        };
        assert_eq!(parts(None), row(0, 600));
        assert_eq!(parts(Some(10)), row(200, 400));
        assert_eq!(parts(Some(15)), row(300, 300));
        assert_eq!(parts(Some(20)), row(400, 200));
        assert_eq!(parts(Some(25)), row(500, 100));
        // A share past the cap is held at the cap.
        assert_eq!(parts(Some(40)), row(500, 100));
    }

    #[test]
    fn four_way_parts_sum_back_and_never_go_negative() {
        let s = sow_split();
        for interest in (0..5_000).chain([99_999, 1_000_001]) {
            for g in [None, Some(10), Some(15), Some(20), Some(25)] {
                let p = split_interest_parts(interest, &s, g);
                let all = [p.platform, p.reserve, p.depositors, p.guarantor, p.recovery_fund];
                assert_eq!(all.iter().sum::<i64>(), interest, "{interest} @ {g:?}");
                assert!(all.iter().all(|v| *v >= 0), "negative part {p:?} for {interest} @ {g:?}");
            }
        }
    }

    const NO_GUARANTORS: &[GuarantorStake] = &[];

    fn stake(pledge: i64, score: i16) -> GuarantorStake {
        GuarantorStake { pledge, score }
    }

    #[test]
    fn depositors_share_pro_rata_across_the_whole_pool() {
        // ₱20.00 of interest; the pool holds ₱3,000, ₱3,000 and ₱2,000. Every
        // depositor earns on their balance, not just whoever funded the loan.
        let booked = book_interest(2_000, &sow_split(), &[300_000, 300_000, 200_000], NO_GUARANTORS);
        assert_eq!(
            booked.parts,
            InterestParts { platform: 200, reserve: 400, depositors: 800, guarantor: 0, recovery_fund: 600 }
        );
        assert_eq!(booked.to_depositors, vec![300, 300, 200]);
        assert!(booked.to_guarantors.is_empty());
    }

    #[test]
    fn guarantor_loans_reproduce_the_published_example() {
        let s = sow_split();
        let pool = [400_000, 400_000];
        // One guarantor backing ₱500 at each tier: ₱2 / ₱3 / ₱4 / ₱5 of ₱20.
        for (score, paid, recovery) in [(50, 200, 400), (80, 300, 300), (105, 400, 200), (130, 500, 100)] {
            let booked = book_interest(2_000, &s, &pool, &[stake(50_000, score)]);
            assert_eq!(booked.parts.guarantor, paid, "score {score}");
            assert_eq!(booked.parts.recovery_fund, recovery, "score {score}");
            assert_eq!(booked.to_guarantors, vec![paid]);
            // Depositors' fixed 40% never moves with the guarantor's tier.
            assert_eq!(booked.parts.depositors, 800);
            assert_eq!(booked.to_depositors, vec![400, 400]);
        }
    }

    #[test]
    fn two_guarantors_earn_their_own_tier_on_their_own_pledge() {
        // Equal pledges at 10% and 25%: 17.5% between them = ₱3.50, split 1:2.5.
        let booked = book_interest(2_000, &sow_split(), &[100_000], &[stake(25_000, 50), stake(25_000, 130)]);
        assert_eq!(booked.parts.guarantor, 350);
        assert_eq!(booked.to_guarantors, vec![100, 250]);
        assert_eq!(booked.guarantor_tiers, vec![10, 25]);
        assert_eq!(booked.parts.recovery_fund, 250);
        // A 3:1 pledge split at the same tier: shared 3:1.
        let booked = book_interest(2_000, &sow_split(), &[100_000], &[stake(30_000, 90), stake(10_000, 90)]);
        assert_eq!(booked.parts.guarantor, 300);
        assert_eq!(booked.to_guarantors, vec![225, 75]);
    }

    #[test]
    fn a_guarantor_below_every_tier_earns_nothing_and_the_fund_keeps_it() {
        let booked = book_interest(2_000, &sow_split(), &[100_000], &[stake(50_000, 30)]);
        assert_eq!(booked.parts.guarantor, 0);
        assert_eq!(booked.to_guarantors, vec![0]);
        assert_eq!(booked.guarantor_tiers, vec![0]);
        assert_eq!(booked.parts.recovery_fund, 600);
    }

    #[test]
    fn with_no_balances_the_depositor_share_stays_in_the_reserve() {
        for balances in [&[][..], &[0, 0][..]] {
            let booked = book_interest(2_000, &sow_split(), balances, NO_GUARANTORS);
            assert_eq!(
                booked.parts,
                InterestParts { platform: 200, reserve: 1_200, depositors: 0, guarantor: 0, recovery_fund: 600 }
            );
            assert!(booked.to_depositors.iter().all(|a| *a == 0));
        }
    }

    #[test]
    fn tiny_interest_still_ties_out() {
        // A centavo or two across three depositors and two guarantors: every
        // part rounds, nothing is lost.
        for interest in 0..=25 {
            let booked = book_interest(interest, &sow_split(), &[1, 1, 1], &[stake(1, 50), stake(1, 150)]);
            let p = booked.parts;
            assert_eq!(p.platform + p.reserve + p.depositors + p.guarantor + p.recovery_fund, interest);
            assert_eq!(booked.to_depositors.iter().sum::<i64>(), p.depositors);
            assert_eq!(booked.to_guarantors.iter().sum::<i64>(), p.guarantor);
        }
    }

    /// Deterministic pseudo-random numbers for the sweeps (no rand dependency).
    fn lcg(seed: u64) -> impl FnMut(u64) -> u64 {
        let mut seed = seed;
        move |max: u64| {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            (seed >> 33) % max.max(1)
        }
    }

    #[test]
    fn every_repayment_shape_balances_the_books() {
        // Interest from a partial payment up to a multi-installment payoff,
        // 0–40 depositors of wildly different sizes, 0–2 guarantors at any
        // score including below every tier.
        let split = sow_split();
        let mut next = lcg(0x5EED);
        for _ in 0..20_000 {
            let interest = next(5_000_000) as i64;
            let balances: Vec<i64> = (0..next(41)).map(|_| next(10_000_000) as i64).collect();
            let guarantors: Vec<GuarantorStake> =
                (0..next(3)).map(|_| stake(1 + next(5_000_000) as i64, next(151) as i16)).collect();

            let booked = book_interest(interest, &split, &balances, &guarantors);
            let p = booked.parts;
            let all = [p.platform, p.reserve, p.depositors, p.guarantor, p.recovery_fund];
            assert_eq!(all.iter().sum::<i64>(), interest, "parts must sum to the interest");
            assert!(all.iter().all(|v| *v >= 0), "negative part {p:?}");
            assert_eq!(booked.to_depositors.len(), balances.len());
            assert_eq!(booked.to_depositors.iter().sum::<i64>(), p.depositors, "depositor lots must equal their credit");
            assert_eq!(booked.to_guarantors.len(), guarantors.len());
            assert_eq!(booked.to_guarantors.iter().sum::<i64>(), p.guarantor, "guarantor lots must equal their credit");
            assert!(booked.to_depositors.iter().chain(&booked.to_guarantors).all(|v| *v >= 0));
            assert!(p.guarantor <= round_half_even(interest as i128 * split.guarantor_cap as i128, 100) + 1);
            // The postings a repayment writes: cash in, and every credit.
            let principal = next(10_000_000) as i64;
            let excess = next(100_000) as i64;
            let cash = principal + interest + excess;
            let credits = principal + p.platform + p.reserve + p.recovery_fund + p.depositors + p.guarantor + excess;
            assert_eq!(cash, credits, "unbalanced repayment");
        }
    }

    #[test]
    fn funding_is_taken_pro_rata_and_never_past_a_balance() {
        // ₱4,000 from ₱3,000 / ₱3,000 / ₱2,000 available: half of each.
        assert_eq!(apportion_capped(400_000, &[300_000, 300_000, 200_000]), vec![150_000, 150_000, 100_000]);
        // Asking for everything takes everything, exactly.
        assert_eq!(apportion_capped(800_000, &[300_000, 300_000, 200_000]), vec![300_000, 300_000, 200_000]);
        // Asking for more than exists takes only what exists.
        assert_eq!(apportion_capped(900_000, &[300_000, 200_000]), vec![300_000, 200_000]);
        // The case half-even + residual-to-largest gets wrong: one centavo short
        // of the whole pool must not push anyone over their own balance.
        let caps = [3, 3, 3];
        let shares = apportion_capped(8, &caps);
        assert_eq!(shares.iter().sum::<i64>(), 8);
        assert!(shares.iter().zip(caps).all(|(s, c)| *s <= c));
        assert!(apportion_capped(0, &[5, 5]).iter().all(|s| *s == 0));
        assert!(apportion_capped(10, &[]).is_empty());
    }

    #[test]
    fn capped_apportioning_never_loses_or_overdraws_a_centavo() {
        let mut next = lcg(0xCA95);
        for _ in 0..20_000 {
            let caps: Vec<i64> = (0..1 + next(40)).map(|_| next(5_000_000) as i64).collect();
            let sum: i64 = caps.iter().sum();
            let total = next(sum as u64 + 10) as i64;
            let shares = apportion_capped(total, &caps);
            assert_eq!(shares.iter().sum::<i64>(), total.min(sum), "{total} across {caps:?}");
            assert!(shares.iter().zip(&caps).all(|(s, c)| *s >= 0 && s <= c), "overdrawn: {shares:?} of {caps:?}");
        }
    }

    #[test]
    fn deposit_backing_decides_what_the_pool_funds() {
        // Deposit-backed: ₱4,500 against ₱5,000 of the borrower's own deposit.
        assert_eq!(pool_funded_amount(450_000, 500_000), 0);
        // XLM collateral: no deposit behind it, so the pool funds it all.
        assert_eq!(pool_funded_amount(450_000, 0), 450_000);
        // Guarantor: ₱3,000 of own deposit cover on a ₱10,000 loan.
        assert_eq!(pool_funded_amount(1_000_000, 300_000), 700_000);
    }

    #[test]
    fn locked_funding_unlocks_with_the_balance_and_all_at_the_end() {
        // ₱7,000 locked on ₱10,000 owed; ₱2,500 of principal comes back.
        assert_eq!(funding_to_release(700_000, 250_000, 1_000_000), 175_000);
        // Nothing paid, nothing released.
        assert_eq!(funding_to_release(700_000, 0, 1_000_000), 0);
        // The final payment releases whatever is left, rounding crumbs included.
        assert_eq!(funding_to_release(123_457, 333, 333), 123_457);
        // Walking a loan to zero in uneven payments never strands a centavo.
        let mut next = lcg(0x10C4);
        for _ in 0..2_000 {
            let principal = 1 + next(10_000_000) as i64;
            let mut locked = next(principal as u64 + 1) as i64;
            let mut outstanding = principal;
            while outstanding > 0 {
                let paid = 1 + next(outstanding as u64) as i64;
                let released = funding_to_release(locked, paid, outstanding);
                assert!((0..=locked).contains(&released));
                locked -= released;
                outstanding -= paid;
            }
            assert_eq!(locked, 0, "funding left locked on a repaid loan");
        }
    }

    #[test]
    fn fee_estimates_match_the_published_rates() {
        let f = paypal_ph();
        // ₱20,000 in: 3.40% = ₱680 + ₱15 = ₱695.
        assert_eq!(receive_fee_estimate(2_000_000, &f), 69_500);
        // ₱5,000 out with the ₱50 cap: ₱4,950 sent, ₱50 charged on top.
        assert_eq!(payout_fee(500_000, &f), 5_000);
        // Never the whole payout.
        assert_eq!(payout_fee(1, &f), 0);
    }

    #[test]
    fn a_payout_plus_its_charge_is_exactly_the_claim() {
        // What the sandbox charged: 2% of what it sent, no cap.
        let uncapped = RailFees { receive_bps: 340, receive_fixed: 1_500, payout_bps: 200, payout_cap: 0 };
        // ₱20,000 claimed: ₱19,607.84 sent, ₱392.16 charged, ₱20,000.00 in all.
        let fee = payout_fee(2_000_000, &uncapped);
        assert_eq!(fee, 39_216);
        assert_eq!(2_000_000 - fee + payout_charge(2_000_000 - fee, &uncapped), 2_000_000);
        // The withdrawal that lost money: sending ₱19,950 costs ₱399 on top.
        assert_eq!(payout_charge(1_995_000, &uncapped), 39_900);

        let mut next = lcg(0x0A70);
        for _ in 0..20_000 {
            let amount = 2 + next(100_000_000) as i64;
            let fees = RailFees {
                receive_bps: 0,
                receive_fixed: 0,
                payout_bps: next(1_000) as i64,
                payout_cap: if next(2) == 0 { 0 } else { next(20_000) as i64 },
            };
            let fee = payout_fee(amount, &fees);
            let sent = amount - fee;
            assert!(sent >= 1 && fee >= 0);
            // Sent plus the provider's charge fits inside the claim...
            assert!(sent + payout_charge(sent, &fees) <= amount, "overdrawn at {amount}");
            // ...and nothing more could have been sent.
            assert!(sent == amount || sent + 1 + payout_charge(sent + 1, &fees) > amount, "sent too little at {amount}");
        }
    }

    #[test]
    fn a_grossed_up_repayment_leaves_exactly_the_installment() {
        let f = paypal_ph();
        // A ₱35,333.00 installment.
        let (total, fee) = gross_up(3_533_300, &f);
        assert_eq!(total, 3_533_300 + fee);
        assert_eq!(total - receive_fee_estimate(total, &f), 3_533_300);
        // And for every amount, the total is the smallest that covers it.
        let mut next = lcg(0xFEE5);
        for _ in 0..20_000 {
            let applies = 1 + next(50_000_000) as i64;
            let fees = RailFees {
                receive_bps: next(1_000) as i64,
                receive_fixed: next(5_000) as i64,
                payout_bps: 0,
                payout_cap: 0,
            };
            let (total, fee) = gross_up(applies, &fees);
            assert_eq!(total, applies + fee);
            assert!(total - receive_fee_estimate(total, &fees) >= applies, "short: {applies} {total}");
            assert!(total - 1 - receive_fee_estimate(total - 1, &fees) < applies, "not smallest: {applies} {total}");
        }
    }

    #[test]
    fn a_bad_fee_table_is_refused() {
        let mut p = params();
        assert_eq!(check_payment_fees(&p), Ok(()));
        p.payment_fees.paypal.receive_bps = 10_000;
        assert!(check_payment_fees(&p).is_err());
        let mut p = params();
        p.payment_fees.stripe.payout_cap = -1;
        assert!(check_payment_fees(&p).is_err());
    }

    #[test]
    fn a_repayment_is_exactly_the_next_unpaid_installment() {
        // (installment, interest_due, interest_paid, principal_due, principal_paid)
        let rows = [
            (1, 2_000, 2_000, 33_333, 33_333), // paid
            (2, 1_333, 500, 33_333, 0),        // interest part-paid
            (3, 667, 0, 33_334, 0),
        ];
        assert_eq!(next_installment_due(&rows), Some((2, 833 + 33_333)));
        // Everything paid: nothing is due, so nothing may be paid.
        assert_eq!(next_installment_due(&[(1, 10, 10, 90, 90)]), None);
        assert_eq!(next_installment_due(&[]), None);
    }

    fn limits_params(tiers: Vec<DepositLimitTier>, below_floor_pct: i64) -> PolicyParams {
        let mut p = params();
        p.deposit_limits = crate::api::lending::policy::DepositLimits { tiers, below_floor_pct };
        p
    }

    fn dtier(min_score: i16, max_score: i16, per_deposit: i64, daily: i64, monthly: i64, max_balance: i64) -> DepositLimitTier {
        DepositLimitTier { min_score, max_score, per_deposit, daily, monthly, max_balance }
    }

    #[test]
    fn below_fifty_gets_half_of_the_lowest_tier() {
        let p = limits_params(
            vec![dtier(50, 69, 2_000_001, 5_000_000, 10_000_000, 20_000_000), dtier(70, 150, 5_000_000, 10_000_000, 25_000_000, 50_000_000)],
            50,
        );
        assert_eq!(check_deposit_limits(&p), Ok(()));
        let entry = deposit_limits_for(50, &p);
        assert_eq!(entry, DepositLimitsFor { per_deposit: 2_000_001, daily: 5_000_000, monthly: 10_000_000, max_balance: 20_000_000 });
        // Score 49 and score 0 alike: half, rounded down (never above half).
        for score in [49, 30, 0] {
            assert_eq!(
                deposit_limits_for(score, &p),
                DepositLimitsFor { per_deposit: 1_000_000, daily: 2_500_000, monthly: 5_000_000, max_balance: 10_000_000 }
            );
        }
        assert_eq!(deposit_limits_for(150, &p).per_deposit, 5_000_000);
    }

    #[test]
    fn a_malformed_deposit_table_is_refused() {
        let ok = || vec![dtier(50, 69, 100, 200, 300, 400), dtier(70, 150, 100, 200, 300, 400)];
        assert_eq!(check_deposit_limits(&limits_params(ok(), 50)), Ok(()));
        let broken = |f: fn(&mut Vec<DepositLimitTier>)| {
            let mut t = ok();
            f(&mut t);
            check_deposit_limits(&limits_params(t, 50))
        };
        assert!(broken(|t| t.clear()).is_err());
        assert!(broken(|t| t[1].min_score = 71).is_err()); // gap
        assert!(broken(|t| t[1].max_score = 149).is_err()); // stops short of 150
        assert!(broken(|t| t[0].per_deposit = 250).is_err()); // per_deposit > daily
        assert!(broken(|t| t[0].daily = 350).is_err()); // daily > monthly
        assert!(broken(|t| t[1].monthly = 250).is_err()); // falls as score rises
        assert!(broken(|t| t[0].max_balance = 0).is_err());
        assert!(check_deposit_limits(&limits_params(ok(), 0)).is_err());
        assert!(check_deposit_limits(&limits_params(ok(), 101)).is_err());
    }

    #[test]
    fn the_allowance_is_the_tightest_remaining_limit() {
        let l = DepositLimitsFor { per_deposit: 2_000_000, daily: 5_000_000, monthly: 10_000_000, max_balance: 20_000_000 };
        assert_eq!(deposit_allowance(l, 0, 0, 0), 2_000_000); // per deposit binds
        assert_eq!(deposit_allowance(l, 4_000_000, 4_000_000, 0), 1_000_000); // 24h binds
        assert_eq!(deposit_allowance(l, 0, 9_500_000, 0), 500_000); // 30 days binds
        assert_eq!(deposit_allowance(l, 0, 0, 19_900_000), 100_000); // balance binds
        assert_eq!(deposit_allowance(l, 6_000_000, 0, 0), 0); // already over: nothing, never negative
        assert_eq!(deposit_allowance(l, 0, 0, 25_000_000), 0); // interest carried them past the cap
    }

    #[test]
    fn scores_map_to_the_sow_guarantor_tiers() {
        let s = sow_split();
        let share = |score| guarantor_tier_for(score, &s).map(|t| t.share);
        assert_eq!(share(49), None);
        assert_eq!(share(50), Some(10));
        assert_eq!(share(79), Some(10));
        assert_eq!(share(80), Some(15));
        assert_eq!(share(104), Some(15));
        assert_eq!(share(105), Some(20));
        assert_eq!(share(129), Some(20));
        assert_eq!(share(130), Some(25));
        assert_eq!(share(150), Some(25));
        assert_eq!(share(151), None);
    }
}
