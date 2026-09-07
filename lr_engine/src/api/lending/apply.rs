//! POST /loans/apply — the shape of every money handler (blueprint §3.2):
//!
//!   L0 transport   require_verified_user (+ CSRF middleware, rate limits)
//!   L1 input       product whitelist, centavos > 0, term in range
//!   L2 domain      PURE checks on locked snapshots: band cap, 90% LTV,
//!                  120% collateral, one-open-loan, the 50% borrower-cover
//!                  floor, pledge coverage
//!   L3 database    row locks + the walls: one-open-loan unique index,
//!                  badge/backing CHECKs, guarantor-cap trigger, balance
//!                  trigger, borrower-cover CHECK
//!
//! The client posts INTENT (product, amount, term, guarantors, and which of
//! its own legs should carry how much); the engine decides legality and price
//! from its own data. Nothing the client sends is believed about money — the
//! cover it asks for is settled against policy by `domain::plan_cover`, the
//! stroops behind an XLM leg are derived from the engine's own agreed rate,
//! and a deposit leg is only real once the lots actually freeze.

use axum::{Extension, Json, http::{HeaderMap, StatusCode}};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::domain;
use super::ledger::{EventDraft, commit_event};
use super::lots;
use super::policy;
use super::pricing;
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
    /// Which of the caller's connected wallets will lock, when a leg of this
    /// loan rests on XLM — the whole of an xlm_collateral loan, or the
    /// borrower's own XLM share of a guarantor loan.
    #[serde(default)]
    wallet_id: Option<Uuid>,
    /// guarantor only: 1..=`guarantors_max` people to invite.
    #[serde(default)]
    guarantors: Vec<GuarantorAsk>,
    /// guarantor only, INTENT: how much of the principal the borrower wants
    /// their own deposit to carry. Centavos.
    #[serde(default)]
    deposit_cover: i64,
    /// guarantor only, INTENT: how much of the principal the borrower wants
    /// their own XLM to carry. Centavos — NOT stroops, and not the
    /// over-collateralized figure. The engine prices it and derives the
    /// stroops that must be locked to carry it at the policy ratio.
    #[serde(default)]
    xlm_cover: i64,
}

#[derive(Serialize)]
pub struct ApplyResponse {
    pub loan_id: Uuid,
    pub status: &'static str,
    pub rate_bps: i32,
    /// The principal recorded, in whole centavos. Echoed back because an
    /// xlm_collateral lock has to name the principal it covers on-chain, and
    /// that number must be the engine's, not the form's.
    pub principal: i64,
    /// xlm_collateral: what the wallet must lock, and where.
    pub required_stroops: Option<i64>,
    pub collateral_contract: Option<String>,
    /// xlm_collateral: the agreed rate that requirement was struck at, when
    /// the feeds were read, and how they were reconciled — pinned, so the
    /// borrower can check the number the engine used rather than trust it.
    pub priced_centavos_per_xlm: Option<i64>,
    pub priced_at: Option<i64>,
    pub price_method: Option<String>,
    /// xlm_collateral: the two legs the vault contract is handed with the
    /// lock — the XLM/USD it measures against Reflector (scaled 1e8) and the
    /// USD/PHP the peso rate was crossed through (centavos) — plus the ratio
    /// it will enforce. The wallet submits these verbatim; the contract
    /// refuses the lock if the feed disagrees or the legs don't support the
    /// peso rate, so there is no number here the client can usefully invent.
    pub priced_usd_per_xlm_e8: Option<i64>,
    pub priced_usd_php_centavos: Option<i64>,
    pub collateral_ratio_bps: Option<i32>,
    /// guarantor: how the loan ended up backed, as the engine settled it.
    /// `cover_required` is the policy floor, the two legs are what the
    /// borrower actually carries, and `guarantor_gap` is what the invitees
    /// must pledge between them. Echoed so the screen shows the engine's
    /// arithmetic rather than repeating its own.
    pub cover_required: Option<i64>,
    pub cover_deposit: Option<i64>,
    pub cover_xlm: Option<i64>,
    pub guarantor_gap: Option<i64>,
    pub message: &'static str,
}

/// What the borrower must lock on chain, and the pinned numbers the wallet
/// submits with the lock.
struct XlmLeg {
    required_stroops: i64,
    contract: String,
    ratio_bps: i32,
    usd_per_xlm_e8: i64,
    usd_php_centavos: i64,
}

/// Opens the borrower's XLM position for `covered_centavos` of principal.
///
/// Extracted because two products now take a leg on coins: an xlm_collateral
/// loan, where the leg carries the whole principal, and a guarantor loan,
/// where it carries whatever share of the borrower's own 50% they chose to
/// put in XLM. The over-collateralization applies to the leg, not to the
/// loan — 120% of what these coins are standing behind — so passing the
/// covered amount is the whole difference between the two callers.
async fn open_xlm_position(
    tx: &mut Transaction<'_, Postgres>,
    loan_id: Uuid,
    user_id: Uuid,
    wallet_id: Option<Uuid>,
    covered_centavos: i64,
    params: &policy::PolicyParams,
    priced: &pricing::Priced,
) -> Result<XlmLeg, E> {
    let contract = stellar::contract_id().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "XLM collateral is not enabled on this deployment yet",
    ))?;
    let wallet_id = wallet_id.ok_or((
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
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| db_err(e, "wallet lookup"))?;
    let address = address.ok_or((
        StatusCode::UNPROCESSABLE_ENTITY,
        "That wallet isn't connected to your account",
    ))?;

    // Both legs, or no loan: the vault contract measures the dollar leg
    // against Reflector and refuses a peso rate the legs don't support, so a
    // quote missing one is a lock the chain would bounce. `for_issuance` has
    // already refused this case — this is the wall behind it, not a second
    // opinion.
    let (usd_per_xlm_e8, usd_php_centavos) = priced.checkable_legs().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "XLM pricing is unavailable — no independent price feeds agree right now",
    ))?;
    let required_stroops = domain::required_collateral_stroops(
        covered_centavos,
        params.xlm_min_collateral_pct,
        priced.centavos_per_xlm,
    );
    // The contract counts the ratio in basis points; policy states it in whole
    // percent. The vault enforces its OWN configured ratio, so if policy is
    // raised without reconfiguring the contract the engine simply asks for
    // more than the chain requires — never less. Lowering policy below the
    // vault's ratio is what would strand borrowers at the lock, and the vault
    // is the wall there by design.
    let ratio_bps = (params.xlm_min_collateral_pct * 100) as i32;

    // The rate is pinned with the position, not looked up again later:
    // "priced at issuance" (SOW §3.8) means a later price move never rewrites
    // what this borrower was asked to lock.
    let collateral_id: Uuid = sqlx::query_scalar(
        "INSERT INTO public.xlm_collateral
            (loan_id, user_id, wallet_address, required_stroops, status,
             priced_centavos_per_xlm, priced_at,
             priced_usd_per_xlm_e8, priced_usd_php_centavos, collateral_ratio_bps)
         VALUES ($1, $2, $3, $4, 'pending', $5, $6, $7, $8, $9)
         RETURNING id",
    )
    .bind(loan_id)
    .bind(user_id)
    .bind(&address)
    .bind(required_stroops)
    .bind(priced.centavos_per_xlm)
    .bind(priced.as_of)
    .bind(usd_per_xlm_e8)
    .bind(usd_php_centavos)
    .bind(ratio_bps)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| db_err(e, "insert collateral"))?;

    // Which feed said what, one row each (027). This is the evidence the
    // borrower's custody record and the proof page read back, and it is kept
    // queryable — "how often did this feed sit outside the band" is a question
    // the database should be able to answer, not one that needs every row
    // unpacked in application code.
    for source in &priced.sources {
        sqlx::query(
            "INSERT INTO public.collateral_price_sources
                (collateral_id, name, centavos_per_xlm, leg, deviation_bps, used)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(collateral_id)
        .bind(&source.name)
        .bind(source.centavos_per_xlm)
        .bind(source.leg)
        .bind(source.deviation_bps)
        .bind(source.used)
        .execute(&mut **tx)
        .await
        .map_err(|e| db_err(e, "insert price source"))?;
    }

    // The summary in the notebook (D9): the agreed number, when it was read,
    // and how much of the principal these coins are standing behind — which on
    // a guarantor loan is a share, not the whole.
    commit_event(
        tx,
        EventDraft {
            kind: "collateral_priced",
            user_id: Some(user_id),
            loan_id: Some(loan_id),
            deposit_id: None,
            rail_ref: None,
            payload: serde_json::json!({
                "centavos_per_xlm": priced.centavos_per_xlm,
                "usd_php_centavos": priced.usd_php_centavos,
                "usd_per_xlm_e8": usd_per_xlm_e8,
                "as_of": priced.as_of,
                "method": priced.method,
                "covered_centavos": covered_centavos,
                "min_collateral_pct": params.xlm_min_collateral_pct,
                "collateral_ratio_bps": ratio_bps,
                "required_stroops": required_stroops,
                "sources_used": priced.sources.iter().filter(|s| s.used).count(),
                "sources_read": priced.sources.len(),
                "unavailable": priced.failures.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
            }),
            actor_id: Some(user_id),
        },
        &[],
    )
    .await
    .map_err(|e| ledger_err(e, "collateral_priced"))?;

    Ok(XlmLeg { required_stroops, contract, ratio_bps, usd_per_xlm_e8, usd_php_centavos })
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

    // XLM collateral is priced from live feeds, and reading feeds is network
    // I/O: it happens BEFORE the transaction opens, never with row locks held
    // (blueprint §3.3, the same rule the Horizon call in collateral.rs
    // follows). Fails closed — no agreement between independent feeds, no
    // loan, rather than a loan struck at a price nobody can vouch for.
    //
    // Two products can rest on XLM now: the whole of an xlm_collateral loan,
    // and the borrower's own share of a guarantor loan when they choose to
    // carry part of it in coins. Both need a price agreed before the
    // transaction opens, and both refuse the loan if no feeds agree.
    let needs_xlm_leg =
        product == "xlm_collateral" || (product == "guarantor" && p.xlm_cover > 0);
    let priced = if needs_xlm_leg {
        // Cheap refusals first: no point asking six providers for a price
        // for a leg this deployment can't issue anyway.
        stellar::contract_id().ok_or((
            StatusCode::SERVICE_UNAVAILABLE,
            "XLM collateral is not enabled on this deployment yet",
        ))?;
        Some(pricing::for_issuance(&pool).await?)
    } else {
        None
    };

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

    // The 50% rule (SOW §4.1), settled before the loan row exists because the
    // borrower's own share is part of what the loan IS. Pure arithmetic on
    // policy and the client's stated intent — the client says how much each of
    // its legs should carry, and every consequence below is derived from the
    // plan this returns, never from a number the client sent.
    let cover = if product == "guarantor" {
        Some(
            domain::plan_cover(amount, p.deposit_cover, p.xlm_cover, params.borrower_cover_min_pct)
                .map_err(|e| match e {
                    // The numbers themselves stay out of the message, as with
                    // the guarantor cap above: the floor is policy data and the
                    // quote endpoint already shows the live figure.
                    domain::CoverError::BelowFloor { .. } => (
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "You must cover at least half of this loan yourself, from your own deposit or your own XLM",
                    ),
                    domain::CoverError::NoGapLeft => (
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "You're covering the whole loan yourself — apply for a deposit-backed or XLM loan instead",
                    ),
                    domain::CoverError::Invalid => (
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "Your cover can't be negative or add up to more than the loan",
                    ),
                })?,
        )
    } else {
        None
    };
    let cover_total = cover.as_ref().map(|c| c.total).unwrap_or(0);

    let loan_id: Uuid = sqlx::query_scalar(
        "INSERT INTO public.loans
            (borrower_id, product, principal, rate_bps, term_months, policy_version,
             borrower_cover_centavos, status)
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'pending')
         RETURNING id",
    )
    .bind(user_id)
    .bind(product)
    .bind(amount)
    .bind(rate)
    .bind(p.term_months)
    .bind(rules.id)
    .bind(cover_total)
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
                principal: amount,
                required_stroops: None,
                collateral_contract: None,
                priced_centavos_per_xlm: None,
                priced_at: None,
                price_method: None,
                priced_usd_per_xlm_e8: None,
                priced_usd_php_centavos: None,
                collateral_ratio_bps: None,
                cover_required: None,
                cover_deposit: None,
                cover_xlm: None,
                guarantor_gap: None,
                message: "Loan approved and disbursed — your backing deposit is locked until it's repaid",
            }
        }
        "xlm_collateral" => {
            let priced = priced.expect("xlm_collateral is priced before the transaction opens");
            // The leg carries the whole principal on this product.
            let leg = open_xlm_position(
                &mut tx, loan_id, user_id, p.wallet_id, amount, &params, &priced,
            )
            .await?;

            ApplyResponse {
                loan_id,
                status: "pending",
                rate_bps: rate,
                principal: amount,
                required_stroops: Some(leg.required_stroops),
                collateral_contract: Some(leg.contract),
                priced_centavos_per_xlm: Some(priced.centavos_per_xlm),
                priced_at: Some(priced.as_of),
                priced_usd_per_xlm_e8: Some(leg.usd_per_xlm_e8),
                priced_usd_php_centavos: Some(leg.usd_php_centavos),
                collateral_ratio_bps: Some(leg.ratio_bps),
                price_method: Some(priced.method),
                cover_required: None,
                cover_deposit: None,
                cover_xlm: None,
                guarantor_gap: None,
                message: "Lock the required XLM from your wallet, then confirm — the loan disburses once the chain shows it",
            }
        }
        "guarantor" => {
            let cover = cover.expect("guarantor cover is planned before the loan row is written");

            // ---- the borrower's own half, before anyone is asked to vouch --
            //
            // Deposit leg first: it settles synchronously, so a borrower who
            // cannot actually cover what they claimed is refused here rather
            // than after invitations have gone out.
            if cover.deposit > 0 {
                lots::freeze_user_lots(&mut tx, user_id, cover.deposit, "collateral", loan_id)
                    .await
                    .map_err(|_| (
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "Not enough withdrawable deposit for the share you're covering yourself",
                    ))?;
            }
            // Coin leg: priced and pinned now, locked on chain afterwards. The
            // 120% ratio applies to this leg alone — the share it stands
            // behind — not to the whole loan.
            let xlm_leg = if cover.xlm > 0 {
                let priced = priced
                    .as_ref()
                    .expect("an XLM cover leg is priced before the transaction opens");
                Some(open_xlm_position(
                    &mut tx, loan_id, user_id, p.wallet_id, cover.xlm, &params, priced,
                )
                .await?)
            } else {
                None
            };

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
            // Guarantors carry only what the borrower did not: the gap left
            // after their own deposit and coins, never the whole principal.
            if total_pledged < cover.guarantor_gap {
                return Err((
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "Guarantor pledges must add up to the share you aren't covering yourself",
                ));
            }

            ApplyResponse {
                loan_id,
                status: "pending",
                rate_bps: rate,
                principal: amount,
                required_stroops: xlm_leg.as_ref().map(|l| l.required_stroops),
                collateral_contract: xlm_leg.as_ref().map(|l| l.contract.clone()),
                priced_centavos_per_xlm: priced.as_ref().map(|q| q.centavos_per_xlm),
                priced_at: priced.as_ref().map(|q| q.as_of),
                price_method: priced.as_ref().map(|q| q.method.clone()),
                priced_usd_per_xlm_e8: xlm_leg.as_ref().map(|l| l.usd_per_xlm_e8),
                priced_usd_php_centavos: xlm_leg.as_ref().map(|l| l.usd_php_centavos),
                collateral_ratio_bps: xlm_leg.as_ref().map(|l| l.ratio_bps),
                cover_required: Some(cover.required),
                cover_deposit: Some(cover.deposit),
                cover_xlm: Some(cover.xlm),
                guarantor_gap: Some(cover.guarantor_gap),
                message: if xlm_leg.is_some() {
                    "Invitations sent — lock your XLM, and the loan disburses once that and your guarantors' pledges are both in"
                } else {
                    "Invitations sent — the loan disburses once your guarantors accept and their pledges cover the rest"
                },
            }
        }
        _ => unreachable!("validate_product whitelists"),
    };

    tx.commit().await.map_err(|e| db_err(e, "commit apply"))?;
    tracing::info!(%user_id, %loan_id, product, amount, "loan application recorded");
    Ok(Json(response))
}
