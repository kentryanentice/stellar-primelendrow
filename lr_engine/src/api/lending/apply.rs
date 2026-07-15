//! POST /loans/apply — the shape of every money handler (blueprint §3.2):
//!
//!   L0 transport   require_verified_user (+ CSRF middleware, rate limits)
//!   L1 input       product whitelist, centavos > 0, term in range
//!   L2 domain      PURE checks on locked snapshots: band cap, 90% LTV,
//!                  120% collateral, one-open-loan, pledge coverage
//!   L3 database    row locks + the walls: one-open-loan unique index,
//!                  badge/backing CHECKs, 3-guarantor trigger, balance trigger
//!
//! The client posts INTENT (product, amount, term, guarantors); the engine
//! decides legality and price from its own data. Nothing the client sends
//! is believed about money.

use axum::{Extension, Json, http::{HeaderMap, StatusCode}};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::domain;
use super::ledger::{EventDraft, commit_event};
use super::lots;
use super::policy;
use super::shared::{db_err, disburse, ledger_err, validate_centavos, validate_product};
use crate::api::users::shared::{E, require_verified_user};
use crate::infra::stellar;

#[derive(Deserialize)]
pub struct GuarantorAsk {
    username: String,
    /// Centavos this guarantor is asked to pledge.
    pledge_amount: i64,
}

#[derive(Deserialize)]
pub struct ApplyInput {
    product: String,
    /// Whole centavos.
    amount: i64,
    term_months: i16,
    /// xlm_collateral only: which of the caller's connected wallets will lock.
    #[serde(default)]
    wallet_id: Option<Uuid>,
    /// guarantor only: 1..=3 people to invite.
    #[serde(default)]
    guarantors: Vec<GuarantorAsk>,
}

#[derive(Serialize)]
pub struct ApplyResponse {
    pub loan_id: Uuid,
    pub status: &'static str,
    pub rate_bps: i32,
    /// xlm_collateral: what the wallet must lock, and where.
    pub required_stroops: Option<i64>,
    pub collateral_contract: Option<String>,
    pub message: &'static str,
}

pub async fn apply(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(p): Json<ApplyInput>,
) -> Result<Json<ApplyResponse>, E> {
    // L0
    let user_id = require_verified_user(&pool, &headers).await?;

    // L1
    let product = validate_product(p.product.trim())?;
    let amount = validate_centavos(p.amount)?;

    let rules = policy::active(&pool).await?;
    let params = rules.params.clone();
    if p.term_months < params.term_months.min || p.term_months > params.term_months.max {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Term must be between 3 and 12 months"));
    }
    if amount < params.min_loan {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Amount is below the minimum loan"));
    }

    let mut tx = pool.begin().await.map_err(|e| db_err(e, "begin apply"))?;

    // L3 first (locks before decisions): the borrower row serializes
    // concurrent applies by the same account; the one-open-loan unique index
    // is the wall behind it.
    sqlx::query("SELECT id FROM public.users WHERE id = $1 FOR UPDATE")
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| db_err(e, "lock borrower"))?;

    let has_open: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM public.loans
          WHERE borrower_id = $1 AND status IN ('pending','active'))",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| db_err(e, "open loan check"))?;
    if has_open {
        return Err((StatusCode::CONFLICT, "You already have an open loan — repay it first"));
    }

    // L2: everything below is a pure function of data we just locked/loaded.
    let score: i16 = sqlx::query_scalar("SELECT score FROM public.credit_scores WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| db_err(e, "credit score"))?
        .unwrap_or(50);
    let band = domain::band_for(score, &params).ok_or((
        StatusCode::FORBIDDEN,
        "Your credit score doesn't qualify for a loan yet",
    ))?;
    let rate = domain::rate_bps(product, band);
    let cap = domain::cap_for(product, band, &params);
    if amount > cap {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Amount exceeds your credit-score cap for this product"));
    }

    let loan_id: Uuid = sqlx::query_scalar(
        "INSERT INTO public.loans
            (borrower_id, product, principal, rate_bps, term_months, policy_version, status)
         VALUES ($1, $2, $3, $4, $5, $6, 'pending')
         RETURNING id",
    )
    .bind(user_id)
    .bind(product)
    .bind(amount)
    .bind(rate)
    .bind(p.term_months)
    .bind(rules.id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        if e.as_database_error().is_some_and(|d| d.is_unique_violation()) {
            // idx_loans_one_open_per_borrower beat a race the row lock missed.
            return (StatusCode::CONFLICT, "You already have an open loan — repay it first");
        }
        db_err(e, "insert loan")
    })?;

    // Consent + application recorded as an event (D1): the exact terms the
    // borrower saw and accepted, in the notebook forever.
    commit_event(
        &mut tx,
        EventDraft {
            kind: "loan_applied",
            user_id: Some(user_id),
            loan_id: Some(loan_id),
            deposit_id: None,
            rail_ref: None,
            payload: serde_json::json!({
                "product": product, "amount": amount, "term_months": p.term_months,
                "rate_bps": rate, "score": score
            }),
            actor_id: Some(user_id),
        },
        &[],
    )
    .await
    .map_err(|e| ledger_err(e, "loan_applied"))?;

    let response = match product {
        "deposit_backed" => {
            // Freeze >= amount/LTV of the borrower's own deposit as collateral,
            // then fund immediately — the pool risks (almost) nothing.
            let required = domain::required_deposit_collateral(amount, params.deposit_ltv_pct);
            lots::freeze_user_lots(&mut tx, user_id, required, "collateral", loan_id)
                .await
                .map_err(|_| (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "Not enough withdrawable deposit — you can borrow up to 90% of what you've deposited",
                ))?;
            disburse(&mut tx, loan_id, user_id, amount, rate, p.term_months).await?;
            ApplyResponse {
                loan_id,
                status: "active",
                rate_bps: rate,
                required_stroops: None,
                collateral_contract: None,
                message: "Loan approved and disbursed — your backing deposit is locked until it's repaid",
            }
        }
        "xlm_collateral" => {
            let contract = stellar::contract_id().ok_or((
                StatusCode::SERVICE_UNAVAILABLE,
                "XLM collateral is not enabled on this deployment yet",
            ))?;
            let wallet_id = p.wallet_id.ok_or((
                StatusCode::UNPROCESSABLE_ENTITY,
                "Choose which connected wallet will lock the collateral",
            ))?;
            // Only a KYC-anchored, ownership-proven wallet may collateralize.
            let address: Option<String> = sqlx::query_scalar(
                "SELECT address FROM public.wallets
                  WHERE id = $1 AND user_id = $2 AND status = 'active'",
            )
            .bind(wallet_id)
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| db_err(e, "wallet lookup"))?;
            let address = address.ok_or((StatusCode::UNPROCESSABLE_ENTITY, "That wallet isn't connected to your account"))?;

            let fx = policy::fx_centavos_per_xlm(&mut *tx).await?;
            let required =
                domain::required_collateral_stroops(amount, params.xlm_min_collateral_pct, fx);

            sqlx::query(
                "INSERT INTO public.xlm_collateral
                    (loan_id, user_id, wallet_address, required_stroops, status)
                 VALUES ($1, $2, $3, $4, 'pending')",
            )
            .bind(loan_id)
            .bind(user_id)
            .bind(&address)
            .bind(required)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_err(e, "insert collateral"))?;

            ApplyResponse {
                loan_id,
                status: "pending",
                rate_bps: rate,
                required_stroops: Some(required),
                collateral_contract: Some(contract),
                message: "Lock the required XLM from your wallet, then confirm — the loan disburses once the chain shows it",
            }
        }
        "guarantor" => {
            if p.guarantors.is_empty() || p.guarantors.len() as i64 > params.guarantors_max {
                // Deliberately doesn't name the exact number: the cap is
                // policy data (D8) and can change (guarantors_max 3 -> 2 in
                // migration 024) without this &'static str being able to
                // follow along — the quote/apply forms already show the
                // live number, this message only needs to explain the shape
                // of the rejection.
                return Err((StatusCode::UNPROCESSABLE_ENTITY, "Invite at least 1 guarantor, up to your policy's guarantor limit"));
            }
            let mut total_pledged: i64 = 0;
            let mut seen: Vec<Uuid> = Vec::new();
            for ask in &p.guarantors {
                let pledge = validate_centavos(ask.pledge_amount)?;
                total_pledged += pledge;
                // A guarantor must be a real, KYC-verified member — the same
                // gate the borrower passed.
                let gid: Option<Uuid> = sqlx::query_scalar(
                    "SELECT id FROM public.users
                      WHERE lower(username) = lower($1) AND role IN ('User','Admin')",
                )
                .bind(ask.username.trim())
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| db_err(e, "guarantor lookup"))?;
                let gid = gid.ok_or((
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "A guarantor wasn't found or isn't verified yet",
                ))?;
                if gid == user_id {
                    return Err((StatusCode::UNPROCESSABLE_ENTITY, "You can't guarantee your own loan"));
                }
                if seen.contains(&gid) {
                    return Err((StatusCode::UNPROCESSABLE_ENTITY, "Each guarantor can only be invited once"));
                }
                seen.push(gid);

                sqlx::query(
                    "INSERT INTO public.loan_guarantors (loan_id, guarantor_id, pledge_amount)
                     VALUES ($1, $2, $3)",
                )
                .bind(loan_id)
                .bind(gid)
                .bind(pledge)
                .execute(&mut *tx)
                .await
                .map_err(|e| db_err(e, "insert guarantor"))?;
            }
            // Guarantors carry the default, so their pledges must cover the
            // whole principal before the loan can fund.
            if total_pledged < amount {
                return Err((
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "Guarantor pledges must add up to at least the loan amount",
                ));
            }

            ApplyResponse {
                loan_id,
                status: "pending",
                rate_bps: rate,
                required_stroops: None,
                collateral_contract: None,
                message: "Invitations sent — the loan disburses once your guarantors accept and their pledges cover it",
            }
        }
        _ => unreachable!("validate_product whitelists"),
    };

    tx.commit().await.map_err(|e| db_err(e, "commit apply"))?;
    tracing::info!(%user_id, %loan_id, product, amount, "loan application recorded");
    Ok(Json(response))
}
