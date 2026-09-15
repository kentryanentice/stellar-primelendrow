//! The versioned rulebook (D8): loads the single active policy_versions row
//! and pins its id onto every loan born under it. Nothing outside this file
//! parses the params JSON — handlers see typed numbers only.

use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::PgExecutor;

use crate::api::users::shared::E;

#[derive(Clone, Deserialize, Serialize)]
pub struct Band {
    pub min_score: i16,
    pub max_score: i16,
    /// Base borrowing cap for the band, centavos.
    pub cap: i64,
    /// Monthly rate for the fully-backed products (deposit_backed, xlm_collateral).
    pub secured_bps: i32,
    /// Monthly rate for guarantor loans — unsecured for the borrower, so priced higher.
    pub guarantor_bps: i32,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct TermRange {
    pub min: i16,
    pub max: i16,
}

/// One row of the score-tier → guarantor-share table (SOW §3/§4). Deliberately
/// NOT a field on `Band`: the SOW's tier cutoffs (50–79, 80–104, 105–129,
/// 130–150) do not line up with the pricing bands, and both are meant to be
/// recalibrated independently.
#[derive(Clone, Deserialize, Serialize)]
pub struct GuarantorTier {
    pub min_score: i16,
    pub max_score: i16,
    /// Percent of the WHOLE interest payment, taken out of the risk band.
    pub share: i64,
}

/// Where a peso of collected interest lands (SOW deliverable 3), in percent of
/// the interest collected. The four fixed shares sum to exactly 100; the
/// guarantor's share is carved out of `risk_band` and whatever is left of the
/// band goes to the recovery fund. `domain::check_interest_split` enforces all
/// of that when the rulebook loads.
#[derive(Clone, Deserialize, Serialize)]
pub struct InterestSplit {
    pub platform: i64,
    pub reserve: i64,
    /// Shared pro-rata across the deposit lots funding the loan. Replaces the
    /// old `savers` field, which was always 0 and never read.
    pub depositors: i64,
    pub risk_band: i64,
    /// Hard ceiling on any tier's share. Must not exceed `risk_band`, so the
    /// recovery fund can be squeezed but never pushed negative.
    pub guarantor_cap: i64,
    /// Contiguous, ascending by score.
    pub guarantor_tiers: Vec<GuarantorTier>,
}

/// One row of the AML deposit-limit table (042), in whole centavos. Its own
/// score cutoffs, like `GuarantorTier`, so compliance can recalibrate deposit
/// limits without moving borrowing caps or guarantor pay.
#[derive(Clone, Deserialize, Serialize)]
pub struct DepositLimitTier {
    pub min_score: i16,
    pub max_score: i16,
    /// The largest single deposit.
    pub per_deposit: i64,
    /// Gross deposits in any rolling 24 hours.
    pub daily: i64,
    /// Gross deposits in any rolling 30 days.
    pub monthly: i64,
    /// The most a member's whole deposit balance may reach through deposits.
    /// Interest credited to them is exempt and may carry them past it.
    pub max_balance: i64,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct DepositLimits {
    /// Contiguous, ascending by score.
    pub tiers: Vec<DepositLimitTier>,
    /// A member scored below the lowest tier gets this percent of that tier's
    /// limits, rounded down (50 = half).
    pub below_floor_pct: i64,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct PolicyParams {
    pub bands: Vec<Band>,
    pub deposit_ltv_pct: i64,
    pub xlm_min_collateral_pct: i64,
    pub xlm_liquidation_pct: i64,
    pub guarantor_cap_multiple: i64,
    pub guarantors_max: i64,
    /// Guarantor loans: the share of the principal the BORROWER must carry
    /// themselves, from their own deposit and/or their own XLM, before any
    /// guarantor is asked for anything (SOW §4.1). A policy parameter, set
    /// conservatively at 50 for this sprint and meant to be recalibrated
    /// against real repayment data — deliberately not a constant.
    pub borrower_cover_min_pct: i64,
    pub term_months: TermRange,
    pub min_deposit: i64,
    pub min_loan: i64,
    pub interest_split: InterestSplit,
    pub deposit_limits: DepositLimits,
}

pub struct Policy {
    pub id: i64,
    pub params: PolicyParams,
}

/// The one active rulebook. A missing or malformed rulebook is a 500, not a
/// silent default — money rules must never be guessed.
pub async fn active<'e, X: PgExecutor<'e>>(executor: X) -> Result<Policy, E> {
    let row: Option<(i64, serde_json::Value)> =
        sqlx::query_as("SELECT id, params FROM public.policy_versions WHERE active")
            .fetch_optional(executor)
            .await
            .map_err(|e| {
                tracing::error!("DB policy load: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Unable to load lending rules")
            })?;

    let (id, params) = row.ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "No active lending policy — run migrations",
    ))?;
    let params: PolicyParams = serde_json::from_value(params).map_err(|e| {
        tracing::error!("policy params malformed: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Unable to load lending rules")
    })?;
    // Well-formed JSON is not enough for the split: shares that don't sum to
    // 100 would silently invent or lose centavos on every repayment.
    super::domain::check_interest_split(&params).map_err(|why| {
        tracing::error!("policy {id} interest_split invalid: {why}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Unable to load lending rules")
    })?;
    // Same for the AML table: a tier whose daily limit is below its per-deposit
    // limit, or a gap in the score cutoffs, would enforce something nobody
    // decided.
    super::domain::check_deposit_limits(&params).map_err(|why| {
        tracing::error!("policy {id} deposit_limits invalid: {why}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Unable to load lending rules")
    })?;
    Ok(Policy { id, params })
}

/// The newest XLM->PHP rate on record (centavos per 1 XLM) and when it was
/// filed. This is HISTORY, not the live price: `pricing::for_display` falls
/// back to it when no feed agrees, and nothing that moves money reads it —
/// issuance goes through `pricing::for_issuance`, which fails closed.
pub async fn last_recorded_fx<'e, X: PgExecutor<'e>>(executor: X) -> Result<(i64, i64), E> {
    sqlx::query_as(
        "SELECT centavos_per_xlm, created_at FROM public.fx_rates
          WHERE base = 'XLM' AND quote = 'PHP'
          ORDER BY created_at DESC, id DESC LIMIT 1",
    )
    .fetch_optional(executor)
    .await
    .map_err(|e| {
        tracing::error!("DB fx rate: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Unable to load exchange rate")
    })?
    .ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "No XLM/PHP rate configured",
    ))
}
