//! GET /loans/{loan_id}/collateral — the custody record for one XLM-collateral
//! loan.
//!
//! Three things a borrower (and, later, the public proof page) should be able
//! to read back without trusting a screen:
//!
//!   * the position — which wallet locked, how much was required, how much the
//!     chain actually shows locked, and what state it is in;
//!   * the price it was struck at, with the feeds behind it — which provider
//!     said what, how far each sat from the agreed number, and which were
//!     dropped; and
//!   * every on-chain movement in order, each with its transaction hash, so
//!     the claim can be checked against the ledger rather than against us.
//!
//! Assembled from what already exists rather than from a new mirror table:
//! the position and its lock hash from `xlm_collateral`, the per-feed evidence
//! from `collateral_price_sources` (027), and the admin-side movements from
//! `collateral_actions`. Every field is a typed column — a custody record
//! whose numbers can't be summed or compared by the database is not much of a
//! record.
//!
//! Nothing here computes money. The only derived numbers are the display
//! value and health, at the live rate.
//!
//! Scoped to the borrower. The vault contract holds no personal data, and
//! neither does this: a wallet address and amounts, never a name.

use axum::{Extension, Json, extract::Path, http::{HeaderMap, StatusCode}};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::domain;
use super::policy;
use super::pricing;
use super::shared::db_err;
use crate::api::users::shared::{E, require_verified_user};
use crate::infra::stellar;

/// One movement of collateral in or out of the vault, in submission order.
#[derive(Serialize)]
pub struct MovementView {
    /// `lock`, `mark_repaid`, `release`, `mark_defaulted` or `seize` — the
    /// contract entry point this movement corresponds to.
    pub kind: String,
    /// `confirmed` once a transaction hash is on record, `queued` while it is
    /// still waiting for the admin key. A queued movement is a claim about
    /// intent, not about the chain, and is labelled as such.
    pub status: &'static str,
    pub tx_hash: Option<String>,
    pub at: Option<i64>,
    /// The checked price submitted with this movement, when it carried one —
    /// a seizure is priced again at the day's quote, and that number decides
    /// how much debt the seized coins cover.
    pub quote_php_per_xlm_centavos: Option<i64>,
    pub quote_usd_per_xlm_e8: Option<i64>,
    pub quote_php_per_usd_centavos: Option<i64>,
}

/// One public feed's contribution to the pinned rate.
#[derive(Serialize)]
pub struct PriceSourceView {
    pub name: String,
    pub centavos_per_xlm: i64,
    /// `XLM/PHP` (quoted directly) or `XLM/USD x USD/PHP` (derived).
    pub leg: String,
    pub deviation_bps: i64,
    /// false = further than the engine's 5% band from the median, so excluded.
    pub used: bool,
}

/// The price the position was struck at, with its evidence.
#[derive(Serialize)]
pub struct PriceEvidence {
    pub centavos_per_xlm: Option<i64>,
    pub usd_per_xlm_e8: Option<i64>,
    pub usd_php_centavos: Option<i64>,
    /// When the feeds were read — not when the row was written.
    pub priced_at: Option<i64>,
    /// How many of the feeds read were inside the band and counted toward the
    /// median. Empty on positions priced before 027 recorded them per feed.
    pub sources_used: usize,
    pub sources: Vec<PriceSourceView>,
}

#[derive(Serialize)]
pub struct CollateralRecord {
    pub loan_id: Uuid,
    pub principal: i64,
    pub loan_status: String,
    /// The vault this deployment locks into, so the record can be checked
    /// against the right contract on an explorer.
    pub contract_id: Option<String>,
    pub wallet_address: String,
    pub required_stroops: i64,
    pub locked_stroops: i64,
    pub status: String,
    pub collateral_ratio_bps: i32,
    pub created_at: i64,
    /// When the lock was verified on-chain; null while still pending.
    pub locked_at: Option<i64>,
    /// What the locked coins are worth right now, at the live rate — display
    /// only. The pinned rate is what the position was struck at.
    pub value_centavos: Option<i64>,
    pub health_pct: Option<i64>,
    pub liquidatable: bool,
    pub price: PriceEvidence,
    pub movements: Vec<MovementView>,
}

#[derive(sqlx::FromRow)]
struct PositionRow {
    id: Uuid,
    wallet_address: String,
    required_stroops: i64,
    locked_stroops: i64,
    status: String,
    lock_tx_hash: Option<String>,
    locked_at: Option<i64>,
    created_at: i64,
    priced_centavos_per_xlm: Option<i64>,
    priced_at: Option<i64>,
    priced_usd_per_xlm_e8: Option<i64>,
    priced_usd_php_centavos: Option<i64>,
    collateral_ratio_bps: i32,
}

pub async fn record(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Path(loan_id): Path<Uuid>,
) -> Result<Json<CollateralRecord>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    // The borrower_id in the WHERE clause IS the authorization: another
    // member's loan id simply doesn't resolve.
    let loan: Option<(i64, String, i64)> = sqlx::query_as(
        "SELECT principal, status, principal_outstanding
           FROM public.loans WHERE id = $1 AND borrower_id = $2",
    )
    .bind(loan_id)
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| db_err(e, "loan lookup"))?;
    let (principal, loan_status, outstanding) =
        loan.ok_or((StatusCode::NOT_FOUND, "No such loan"))?;

    let position: Option<PositionRow> = sqlx::query_as(
        "SELECT id, wallet_address, required_stroops, locked_stroops, status,
                lock_tx_hash, locked_at, created_at,
                priced_centavos_per_xlm, priced_at, priced_usd_per_xlm_e8,
                priced_usd_php_centavos, collateral_ratio_bps
           FROM public.xlm_collateral WHERE loan_id = $1",
    )
    .bind(loan_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| db_err(e, "collateral position"))?;
    let position = position.ok_or((
        StatusCode::NOT_FOUND,
        "That loan has no XLM collateral against it",
    ))?;

    let sources: Vec<(String, i64, String, i64, bool)> = sqlx::query_as(
        "SELECT name, centavos_per_xlm, leg, deviation_bps, used
           FROM public.collateral_price_sources
          WHERE collateral_id = $1
          ORDER BY used DESC, deviation_bps, name",
    )
    .bind(position.id)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "price sources"))?;

    let actions: Vec<(String, String, Option<String>, i64, Option<i64>, Option<i64>, Option<i64>, Option<i64>)> =
        sqlx::query_as(
            "SELECT action, status, tx_hash, created_at, done_at,
                    quote_php_per_xlm_centavos, quote_usd_per_xlm_e8,
                    quote_php_per_usd_centavos
               FROM public.collateral_actions
              WHERE collateral_id = $1
              ORDER BY id",
        )
        .bind(position.id)
        .fetch_all(&pool)
        .await
        .map_err(|e| db_err(e, "collateral movements"))?;

    // Display numbers only, at the live rate — `for_display` degrades rather
    // than failing, because a screen is not money.
    let rules = policy::active(&pool).await?;
    let fx = pricing::for_display(&pool).await?.centavos_per_xlm;
    let (value_centavos, health_pct) = if position.locked_stroops > 0 {
        let value = domain::collateral_value_centavos(position.locked_stroops, fx);
        (Some(value), (outstanding > 0).then(|| value * 100 / outstanding))
    } else {
        (None, None)
    };
    let liquidatable = position.status == "locked"
        && health_pct.is_some_and(|h| h < rules.params.xlm_liquidation_pct);

    // The lock comes first and is the borrower's own transaction; everything
    // after it is admin-signed and comes off the outbox in submission order.
    let mut movements = Vec::with_capacity(actions.len() + 1);
    if let Some(tx_hash) = position.lock_tx_hash.clone() {
        movements.push(MovementView {
            kind: "lock".to_string(),
            status: "confirmed",
            tx_hash: Some(tx_hash),
            at: position.locked_at,
            // The lock's price is the pinned one under `price`; repeating it
            // here would suggest two different numbers had been checked.
            quote_php_per_xlm_centavos: None,
            quote_usd_per_xlm_e8: None,
            quote_php_per_usd_centavos: None,
        });
    }
    for (kind, status, tx_hash, created_at, done_at, php_xlm, usd_xlm, php_usd) in actions {
        let done = status == "done";
        movements.push(MovementView {
            kind,
            status: if done { "confirmed" } else { "queued" },
            tx_hash,
            at: if done { done_at.or(Some(created_at)) } else { Some(created_at) },
            quote_php_per_xlm_centavos: php_xlm,
            quote_usd_per_xlm_e8: usd_xlm,
            quote_php_per_usd_centavos: php_usd,
        });
    }

    let sources: Vec<PriceSourceView> = sources
        .into_iter()
        .map(|(name, centavos_per_xlm, leg, deviation_bps, used)| PriceSourceView {
            name, centavos_per_xlm, leg, deviation_bps, used,
        })
        .collect();

    Ok(Json(CollateralRecord {
        loan_id,
        principal,
        loan_status,
        contract_id: stellar::contract_id(),
        wallet_address: position.wallet_address,
        required_stroops: position.required_stroops,
        locked_stroops: position.locked_stroops,
        status: position.status,
        collateral_ratio_bps: position.collateral_ratio_bps,
        created_at: position.created_at,
        locked_at: position.locked_at,
        value_centavos,
        health_pct,
        liquidatable,
        price: PriceEvidence {
            centavos_per_xlm: position.priced_centavos_per_xlm,
            usd_per_xlm_e8: position.priced_usd_per_xlm_e8,
            usd_php_centavos: position.priced_usd_php_centavos,
            priced_at: position.priced_at,
            sources_used: sources.iter().filter(|s| s.used).count(),
            sources,
        },
        movements,
    }))
}
