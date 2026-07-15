//! GET /loans/quote — the engine's numbers, pre-computed for display.
//!
//! The frontend never computes a price, a cap, or a collateral requirement;
//! it asks this endpoint and shows the answer verbatim (blueprint §4). The
//! apply handler re-derives everything anyway — a tampered quote changes a
//! screen, never a loan.

use axum::{Extension, Json, extract::Query, http::HeaderMap};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use super::domain;
use super::policy;
use super::shared::db_err;
use crate::api::users::shared::{E, require_verified_user};

#[derive(Deserialize)]
pub struct QuoteQuery {
    /// Optional: omitted = "just show me my maxes".
    #[serde(default)]
    amount: Option<i64>,
    #[serde(default)]
    term_months: Option<i16>,
    /// Which product the schedule preview should be priced for.
    #[serde(default)]
    product: Option<String>,
}

#[derive(Serialize)]
pub struct ProductQuote {
    pub product: &'static str,
    pub eligible: bool,
    pub reason: Option<&'static str>,
    pub rate_bps: i32,
    pub max_amount: i64,
    /// deposit_backed: centavos of your deposit that would freeze.
    pub required_deposit: Option<i64>,
    /// xlm_collateral: stroops that must be locked on-chain.
    pub required_stroops: Option<i64>,
    /// guarantor: total pledges your guarantors must cover.
    pub required_pledges: Option<i64>,
}

#[derive(Serialize)]
pub struct InstallmentPreview {
    pub installment: i16,
    pub principal_due: i64,
    pub interest_due: i64,
}

#[derive(Serialize)]
pub struct QuoteResponse {
    pub score: i16,
    pub band_cap: Option<i64>,
    pub eligible: bool,
    pub products: Vec<ProductQuote>,
    /// Present when amount+term were given and within range.
    pub schedule_preview: Option<Vec<InstallmentPreview>>,
    pub total_interest: Option<i64>,
}

pub async fn quote(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Query(q): Query<QuoteQuery>,
) -> Result<Json<QuoteResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    let rules = policy::active(&pool).await?;
    let params = &rules.params;
    let fx = policy::fx_centavos_per_xlm(&pool).await?;

    let score: i16 = sqlx::query_scalar("SELECT score FROM public.credit_scores WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| db_err(e, "credit score"))?
        .unwrap_or(50);

    let available: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0)::BIGINT FROM public.deposits
          WHERE user_id = $1 AND badge = 'available'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| db_err(e, "available deposits"))?;

    let has_open_loan: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM public.loans
          WHERE borrower_id = $1 AND status IN ('pending','active'))",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| db_err(e, "open loan check"))?;

    let band = domain::band_for(score, params);

    let mut products = Vec::with_capacity(3);
    if let Some(band) = band {
        let blocked = if has_open_loan { Some("You already have an open loan — repay it first") } else { None };

        // deposit_backed
        let dep_cap = domain::cap_for("deposit_backed", band, params)
            .min(domain::max_borrow_against_deposit(available, params.deposit_ltv_pct));
        products.push(ProductQuote {
            product: "deposit_backed",
            eligible: blocked.is_none() && dep_cap >= params.min_loan,
            reason: blocked.or(if dep_cap < params.min_loan { Some("Deposit more first — you can borrow up to 90% of your withdrawable deposit") } else { None }),
            rate_bps: domain::rate_bps("deposit_backed", band),
            max_amount: dep_cap,
            required_deposit: q.amount.map(|a| domain::required_deposit_collateral(a, params.deposit_ltv_pct)),
            required_stroops: None,
            required_pledges: None,
        });

        // xlm_collateral
        let contract_ready = crate::infra::stellar::contract_id().is_some();
        products.push(ProductQuote {
            product: "xlm_collateral",
            eligible: blocked.is_none() && contract_ready,
            reason: blocked.or(if contract_ready { None } else { Some("XLM collateral is not enabled on this deployment yet") }),
            rate_bps: domain::rate_bps("xlm_collateral", band),
            max_amount: domain::cap_for("xlm_collateral", band, params),
            required_deposit: None,
            required_stroops: q
                .amount
                .map(|a| domain::required_collateral_stroops(a, params.xlm_min_collateral_pct, fx)),
            required_pledges: None,
        });

        // guarantor
        products.push(ProductQuote {
            product: "guarantor",
            eligible: blocked.is_none(),
            reason: blocked,
            rate_bps: domain::rate_bps("guarantor", band),
            max_amount: domain::cap_for("guarantor", band, params),
            required_deposit: None,
            required_stroops: None,
            required_pledges: q.amount,
        });
    }

    // Schedule preview only when the inputs are a real, in-range ask — the
    // apply handler re-validates all of it regardless.
    let (schedule_preview, total_interest) = match (q.amount, q.term_months, band) {
        (Some(amount), Some(term), Some(band))
            if amount > 0
                && term >= params.term_months.min
                && term <= params.term_months.max =>
        {
            let product = q.product.as_deref().unwrap_or("deposit_backed");
            let rows = domain::build_schedule(amount, domain::rate_bps(product, band), term, 0);
            let total = rows.iter().map(|r| r.interest_due).sum();
            (
                Some(
                    rows.into_iter()
                        .map(|r| InstallmentPreview {
                            installment: r.installment,
                            principal_due: r.principal_due,
                            interest_due: r.interest_due,
                        })
                        .collect(),
                ),
                Some(total),
            )
        }
        _ => (None, None),
    };

    Ok(Json(QuoteResponse {
        score,
        band_cap: band.map(|b| b.cap),
        eligible: band.is_some(),
        products,
        schedule_preview,
        total_interest,
    }))
}
