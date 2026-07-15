//! GET /pool — the single read that drives the whole Lending page: the
//! pool's honest utilization numbers, the caller's own four numbers with
//! their lots, and the engine's parameters (rules, fx, contract) so the
//! frontend never has to invent any of them (Lesson 3: the UI is a window,
//! never a calculator).

use axum::{Extension, Json, http::HeaderMap};
use serde::Serialize;
use sqlx::PgPool;

use super::ledger::account_balance;
use super::policy::{self, PolicyParams};
use super::shared::db_err;
use crate::api::users::shared::{E, require_verified_user};
use crate::infra::{paypal, stellar};

#[derive(Serialize)]
pub struct PoolStats {
    pub total_deposits: i64,
    pub cash_available: i64,
    pub out_on_loans: i64,
    pub active_loans: i64,
    /// 0..100, integer — how much of the pool is working.
    pub utilization_pct: i64,
}

#[derive(Serialize)]
pub struct MyFunds {
    pub available: i64,
    pub lent: i64,
    pub collateral: i64,
    pub pledged: i64,
    pub score: i16,
}

#[derive(Serialize)]
pub struct Params {
    pub policy: PolicyParams,
    pub fx_centavos_per_xlm: i64,
    pub collateral_contract: Option<String>,
    pub paypal_ready: bool,
}

#[derive(Serialize)]
pub struct PoolResponse {
    pub pool: PoolStats,
    pub me: MyFunds,
    pub params: Params,
}

pub async fn summary(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
) -> Result<Json<PoolResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    let rules = policy::active(&pool).await?;
    let fx = policy::fx_centavos_per_xlm(&pool).await?;

    // ::BIGINT everywhere SUM appears: Postgres widens SUM(BIGINT) to NUMERIC,
    // which sqlx refuses to decode as i64.
    let total_deposits: i64 =
        sqlx::query_scalar("SELECT COALESCE(SUM(amount), 0)::BIGINT FROM public.deposits")
            .fetch_one(&pool)
            .await
            .map_err(|e| db_err(e, "pool totals"))?;

    let cash_available = account_balance(&pool, "cash")
        .await
        .map_err(|e| db_err(e, "cash balance"))?;

    let (out_on_loans, active_loans): (i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(principal_outstanding), 0)::BIGINT, COUNT(*)
           FROM public.loans WHERE status = 'active'",
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| db_err(e, "loan totals"))?;

    let working = out_on_loans + cash_available;
    let utilization_pct = if working > 0 { out_on_loans * 100 / working } else { 0 };

    // The four running totals, grouped in the database rather than summed by
    // looping the caller's full lot list — the list itself now lives behind
    // its own paginated endpoint (deposits_list) and this response no longer
    // fetches it.
    let badge_totals: Vec<(String, i64)> = sqlx::query_as(
        "SELECT badge, COALESCE(SUM(amount), 0)::BIGINT
           FROM public.deposits
          WHERE user_id = $1
          GROUP BY badge",
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "my badge totals"))?;

    let mut me = MyFunds {
        available: 0,
        lent: 0,
        collateral: 0,
        pledged: 0,
        score: 50,
    };
    for (badge, amount) in badge_totals {
        match badge.as_str() {
            "available" => me.available = amount,
            "lent" => me.lent = amount,
            "collateral" => me.collateral = amount,
            "pledged" => me.pledged = amount,
            _ => {}
        }
    }

    me.score = sqlx::query_scalar("SELECT score FROM public.credit_scores WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| db_err(e, "credit score"))?
        .unwrap_or(50);

    Ok(Json(PoolResponse {
        pool: PoolStats {
            total_deposits,
            cash_available,
            out_on_loans,
            active_loans,
            utilization_pct,
        },
        me,
        params: Params {
            policy: rules.params,
            fx_centavos_per_xlm: fx,
            collateral_contract: stellar::contract_id(),
            paypal_ready: paypal::is_configured(),
        },
    }))
}
