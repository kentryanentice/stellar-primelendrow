//! The collateral outbox, drained at last.
//!
//! POST /lending/admin/actions          — what is queued, admin-only.
//! POST /lending/admin/actions/prepare  — pin a movement's parameters for signing.
//! POST /lending/admin/actions/confirm  — record what the chain did with it.
//!
//! `collateral_actions` has existed since 022 with a comment promising "the
//! operator (or a future signer job) executes the contract call with the admin
//! key and marks the row done". Nothing ever did, so a repaid loan's release
//! sat queued forever and a seizure had no path at all. This module is that
//! executor's engine half, shaped like the lock rather than like a worker:
//!
//!   prepare -> the admin signs in Freighter and submits -> confirm
//!
//! which is the same three beats `apply -> lock -> /collateral/confirm` takes
//! for a borrower, for the same reason. The key that can move coins out of the
//! vault is the one thing a compromised server must not be able to use, so it
//! stays in the admin's wallet and never reaches this process.
//!
//! **The engine picks the numbers, not the admin.** `prepare` pins a freshly
//! agreed quote onto the seizure row and hands back the treasury address from
//! the environment. `confirm` reads both back off the row — it does not accept
//! a price or a destination from the caller, so the figure a seizure applies
//! to the borrower's debt is one the engine agreed and the contract checked,
//! not one an administrator typed.

use axum::{Extension, Json, http::{HeaderMap, StatusCode}};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::domain;
use super::pricing;
use super::recovery;
use super::shared::db_err;
use crate::api::users::shared::{E, require_admin};
use crate::infra::stellar;

#[derive(Serialize)]
pub struct QueuedAction {
    pub id: i64,
    pub loan_id: Uuid,
    /// `mark_repaid`, `release`, `mark_defaulted` or `seize`.
    pub action: String,
    /// The borrower whose position this is — shown so an operator signing a
    /// seizure can see whose coins are about to move.
    pub borrower: String,
    pub wallet_address: String,
    pub locked_stroops: i64,
    /// Set once prepared: the rate this seizure will be valued at.
    pub quote_php_per_xlm_centavos: Option<i64>,
    pub created_at: i64,
}

#[derive(Serialize)]
pub struct ActionsResponse {
    pub actions: Vec<QueuedAction>,
    /// The vault this deployment signs against; null means the deployment has
    /// no contract configured and nothing here can be executed.
    pub contract_id: Option<String>,
    pub treasury: Option<String>,
}

type QueuedRow = (i64, Uuid, String, String, String, i64, Option<i64>, i64);

const QUEUED_COLUMNS: &str = "a.id, c.loan_id, a.action, u.username, c.wallet_address,
                              c.locked_stroops, a.quote_php_per_xlm_centavos, a.created_at";

const QUEUED_FROM: &str = "FROM public.collateral_actions a
                           JOIN public.xlm_collateral c ON c.id = a.collateral_id
                           JOIN public.users u ON u.id = c.user_id";

fn view(row: QueuedRow) -> QueuedAction {
    let (id, loan_id, action, borrower, wallet_address, locked_stroops, quote_php_per_xlm_centavos, created_at) = row;
    QueuedAction { id, loan_id, action, borrower, wallet_address, locked_stroops, quote_php_per_xlm_centavos, created_at }
}

/// Everything still waiting for the admin key, oldest first. Queue order IS
/// execution order: the contract refuses `release` before `mark_repaid` and
/// `seize` before `mark_defaulted`, so the id ordering is not cosmetic.
pub async fn list(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
) -> Result<Json<ActionsResponse>, E> {
    require_admin(&pool, &headers).await?;

    let rows: Vec<QueuedRow> = sqlx::query_as(&format!(
        "SELECT {QUEUED_COLUMNS} {QUEUED_FROM}
          WHERE a.status = 'queued'
          ORDER BY a.id
          LIMIT 100"
    ))
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "queued actions"))?;

    Ok(Json(ActionsResponse {
        actions: rows.into_iter().map(view).collect(),
        contract_id: stellar::contract_id(),
        treasury: stellar::treasury_address().ok(),
    }))
}

#[derive(Deserialize)]
pub struct ActionInput {
    action_id: i64,
}

#[derive(Serialize)]
pub struct PreparedAction {
    pub id: i64,
    pub loan_id: Uuid,
    pub action: String,
    pub contract_id: String,
    /// Only a seizure needs somewhere to send coins; a release's destination
    /// is the depositor the contract recorded at lock time and cannot be
    /// influenced from here at all.
    pub treasury: Option<String>,
    /// The three numbers the vault's `Quote` takes. Present for a seizure,
    /// absent for the three movements that carry no price.
    pub quote: Option<Quote>,
    pub message: &'static str,
}

#[derive(Serialize)]
pub struct Quote {
    pub php_per_xlm_centavos: i64,
    pub usd_per_xlm_e8: i64,
    pub php_per_usd_centavos: i64,
}

/// Pins what a movement will be signed with. For a seizure that means a fresh
/// agreed rate written onto the row — after this the price is the engine's
/// record, so `confirm` never has to believe the caller about what the coins
/// were worth.
///
/// Idempotent: preparing twice re-prices, which is correct — a quote the
/// contract would now reject as stale is no use to anyone, and nothing has
/// been signed against the old one yet.
pub async fn prepare(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(p): Json<ActionInput>,
) -> Result<Json<PreparedAction>, E> {
    require_admin(&pool, &headers).await?;
    let contract_id = stellar::contract_id().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "No vault contract is configured on this deployment",
    ))?;

    let row: Option<(String, Uuid, String, Uuid)> = sqlx::query_as(
        "SELECT a.action, c.loan_id, a.status, a.collateral_id
           FROM public.collateral_actions a
           JOIN public.xlm_collateral c ON c.id = a.collateral_id
          WHERE a.id = $1",
    )
    .bind(p.action_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| db_err(e, "action lookup"))?;
    let (action, loan_id, status, collateral_id) =
        row.ok_or((StatusCode::NOT_FOUND, "No such movement"))?;
    if status != "queued" {
        return Err((StatusCode::CONFLICT, "That movement has already been executed"));
    }

    // Queue order is execution order, and the contract is the one enforcing
    // it: `release` is refused until a repayment is recorded, `seize` until a
    // default is. Signing out of order therefore doesn't quietly do the wrong
    // thing — it burns a transaction on a refusal the operator then has to
    // decode. Better to refuse before anyone signs, and say why.
    let pending_before: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM public.collateral_actions
          WHERE collateral_id = $1 AND status = 'queued' AND id < $2
          ORDER BY id LIMIT 1",
    )
    .bind(collateral_id)
    .bind(p.action_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| db_err(e, "predecessor lookup"))?;
    if pending_before.is_some() {
        return Err((
            StatusCode::CONFLICT,
            "An earlier movement on this position hasn't been recorded on-chain yet — the vault will refuse this one until it is. Sign them in the order they're listed",
        ));
    }

    if action != "seize" {
        return Ok(Json(PreparedAction {
            id: p.action_id,
            loan_id,
            action,
            contract_id,
            treasury: None,
            quote: None,
            message: "Sign this movement in your wallet, then confirm it with the transaction hash",
        }));
    }

    let treasury = stellar::treasury_address()
        .map_err(|reason| (StatusCode::SERVICE_UNAVAILABLE, reason))?;

    // Fail-closed, exactly as issuing a loan is: the vault measures the dollar
    // leg against Reflector and refuses a stale or out-of-band number, so
    // sending the admin to sign against a price the engine can't stand behind
    // would only waste a transaction.
    let priced = pricing::for_issuance(&pool).await?;
    let (usd_per_xlm_e8, php_per_usd_centavos) = priced
        .checkable_legs()
        .ok_or((StatusCode::SERVICE_UNAVAILABLE, "No checkable XLM price right now"))?;

    sqlx::query(
        "UPDATE public.collateral_actions
            SET quote_php_per_xlm_centavos = $1,
                quote_usd_per_xlm_e8 = $2,
                quote_php_per_usd_centavos = $3
          WHERE id = $4 AND status = 'queued'",
    )
    .bind(priced.centavos_per_xlm)
    .bind(usd_per_xlm_e8)
    .bind(php_per_usd_centavos)
    .bind(p.action_id)
    .execute(&pool)
    .await
    .map_err(|e| db_err(e, "pin seizure quote"))?;

    Ok(Json(PreparedAction {
        id: p.action_id,
        loan_id,
        action,
        contract_id,
        treasury: Some(treasury),
        quote: Some(Quote {
            php_per_xlm_centavos: priced.centavos_per_xlm,
            usd_per_xlm_e8,
            php_per_usd_centavos,
        }),
        message: "Sign the seizure in your wallet, then confirm it with the transaction hash",
    }))
}

#[derive(Deserialize)]
pub struct ConfirmInput {
    action_id: i64,
    tx_hash: String,
}

#[derive(Serialize)]
pub struct ConfirmResponse {
    pub id: i64,
    /// What Horizon showed leaving the vault; null for the two movements that
    /// record an outcome without moving coins.
    pub moved_stroops: Option<i64>,
    /// What those coins covered, for a seizure.
    pub value_centavos: Option<i64>,
    pub shortfall: Option<i64>,
    pub message: &'static str,
}

pub async fn confirm(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(p): Json<ConfirmInput>,
) -> Result<Json<ConfirmResponse>, E> {
    let admin_id = require_admin(&pool, &headers).await?;
    let contract_id = stellar::contract_id().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "No vault contract is configured on this deployment",
    ))?;
    let tx_hash = p.tx_hash.trim().to_string();

    let row: Option<(String, String, Uuid, Uuid, Uuid, Option<i64>)> = sqlx::query_as(
        "SELECT a.action, a.status, a.collateral_id, c.loan_id, c.user_id,
                a.quote_php_per_xlm_centavos
           FROM public.collateral_actions a
           JOIN public.xlm_collateral c ON c.id = a.collateral_id
          WHERE a.id = $1",
    )
    .bind(p.action_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| db_err(e, "action lookup"))?;
    let (action, status, collateral_id, loan_id, borrower_id, pinned_rate) =
        row.ok_or((StatusCode::NOT_FOUND, "No such movement"))?;
    if status != "queued" {
        return Err((StatusCode::CONFLICT, "That movement has already been executed"));
    }

    // Horizon first, outside any transaction — no database lock is ever held
    // across a network round trip.
    let moved = match action.as_str() {
        "release" | "seize" => Some(
            stellar::verify_vault_movement(&tx_hash, &contract_id)
                .await
                .map_err(|m| (StatusCode::UNPROCESSABLE_ENTITY, m))?,
        ),
        _ => {
            stellar::verify_contract_call(&tx_hash, &action)
                .await
                .map_err(|m| (StatusCode::UNPROCESSABLE_ENTITY, m))?;
            None
        }
    };

    // A seizure's destination is checked, not assumed: the contract takes the
    // address from its caller, so this is the one movement where a wrong
    // destination is possible and has to be caught.
    if action == "seize" {
        let treasury = stellar::treasury_address()
            .map_err(|reason| (StatusCode::SERVICE_UNAVAILABLE, reason))?;
        if moved.as_ref().is_some_and(|m| m.to != treasury) {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                "Those coins didn't go to the treasury address — that seizure will not be recorded",
            ));
        }
    }

    let stroops = moved.as_ref().map(|m| m.stroops);
    // The rate is the one pinned at prepare time; the caller has no say in it.
    let value_centavos = match (action.as_str(), stroops, pinned_rate) {
        ("seize", Some(stroops), Some(rate)) => Some(domain::collateral_value_centavos(stroops, rate)),
        ("seize", _, None) => {
            return Err((
                StatusCode::CONFLICT,
                "That seizure was never prepared, so there is no checked price to value it at",
            ));
        }
        _ => None,
    };

    let mut tx = pool.begin().await.map_err(|e| db_err(e, "begin confirm"))?;
    let now = Utc::now().timestamp();

    // Claim the row first. `status = 'queued'` in the WHERE plus the unique
    // index on tx_hash means a double click, or one hash aimed at two
    // movements, settles exactly one of them.
    let claimed = sqlx::query(
        "UPDATE public.collateral_actions
            SET status = 'done', tx_hash = $1, done_at = $2,
                moved_stroops = $3, value_centavos = $4
          WHERE id = $5 AND status = 'queued'",
    )
    .bind(&tx_hash)
    .bind(now)
    .bind(stroops)
    .bind(value_centavos)
    .bind(p.action_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        if e.as_database_error().is_some_and(|d| d.is_unique_violation()) {
            // idx_collateral_actions_tx: that transaction already settled a
            // movement, so this one isn't it.
            return (StatusCode::CONFLICT, "That transaction has already been recorded against a movement");
        }
        db_err(e, "settle action")
    })?;
    if claimed.rows_affected() == 0 {
        return Err((StatusCode::CONFLICT, "That movement has already been executed"));
    }

    // Only now does the position's claim move — the DB follows the chain, it
    // never runs ahead of it.
    let mut shortfall = None;
    match action.as_str() {
        "release" => {
            sqlx::query(
                "UPDATE public.xlm_collateral SET status = 'released', updated_at = $1
                  WHERE id = $2 AND status = 'locked'",
            )
            .bind(now)
            .bind(collateral_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_err(e, "mark released"))?;
        }
        "seize" => {
            sqlx::query(
                "UPDATE public.xlm_collateral SET status = 'seized', updated_at = $1
                  WHERE id = $2 AND status = 'locked'",
            )
            .bind(now)
            .bind(collateral_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_err(e, "mark seized"))?;

            // The coins are gone and worth what the contract checked; that
            // value pays down the debt, and the waterfall picks up where it
            // paused — guarantors are charged for the remainder, if any.
            if let (Some(stroops), Some(value)) = (stroops, value_centavos) {
                recovery::record_seizure(&mut tx, loan_id, borrower_id, stroops, value, admin_id).await?;
            }
            shortfall = Some(recovery::advance(&mut tx, loan_id, admin_id).await?.shortfall);
        }
        _ => {}
    }

    tx.commit().await.map_err(|e| db_err(e, "commit confirm"))?;
    tracing::info!(%admin_id, action_id = p.action_id, %action, ?stroops, "vault movement confirmed");

    Ok(Json(ConfirmResponse {
        id: p.action_id,
        moved_stroops: stroops,
        value_centavos,
        shortfall,
        message: match action.as_str() {
            "release" => "Release confirmed on-chain — the collateral is back with the borrower",
            "seize" => "Seizure confirmed on-chain and applied to the debt",
            _ => "Outcome recorded on-chain",
        },
    }))
}
