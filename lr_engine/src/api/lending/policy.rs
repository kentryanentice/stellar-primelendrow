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

#[derive(Clone, Deserialize, Serialize)]
pub struct InterestSplit {
    pub savers: i64,
    pub platform: i64,
    pub reserve: i64,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct PolicyParams {
    pub bands: Vec<Band>,
    pub deposit_ltv_pct: i64,
    pub xlm_min_collateral_pct: i64,
    pub xlm_liquidation_pct: i64,
    pub guarantor_cap_multiple: i64,
    pub guarantors_max: i64,
    pub term_months: TermRange,
    pub min_deposit: i64,
    pub min_loan: i64,
    pub interest_split: InterestSplit,
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
