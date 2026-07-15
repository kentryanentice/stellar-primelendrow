//! POST /collateral/confirm — turn an on-chain lock into a disbursed loan.
//!
//! The client hands over nothing but its pending loan id and the hash of the
//! lock transaction it submitted. The engine re-derives the truth from
//! Horizon (infra::stellar): success, our contract, a native transfer from
//! the wallet pinned at apply time, amount >= the requirement pinned at
//! apply time. The verified stroops — not the client's claim — become the
//! recorded collateral, and the unique lock_tx_hash makes a replayed
//! confirmation bounce off the schema.

use axum::{Extension, Json, http::{HeaderMap, StatusCode}};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::ledger::{EventDraft, commit_event};
use super::shared::{db_err, disburse, ledger_err};
use crate::api::users::shared::{E, require_verified_user};
use crate::infra::stellar;

#[derive(Deserialize)]
pub struct ConfirmInput {
    loan_id: Uuid,
    tx_hash: String,
}

#[derive(Serialize)]
pub struct ConfirmResponse {
    pub loan_id: Uuid,
    pub status: &'static str,
    pub locked_stroops: i64,
    pub message: &'static str,
}

pub async fn confirm(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(p): Json<ConfirmInput>,
) -> Result<Json<ConfirmResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;
    let contract = stellar::contract_id().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "XLM collateral is not enabled on this deployment yet",
    ))?;

    // Read the pinned expectations WITHOUT locks first: the Horizon
    // round-trip must never hold row locks open (blueprint §3.3).
    let pinned: Option<(String, i64, String)> = sqlx::query_as(
        "SELECT c.wallet_address, c.required_stroops, c.status
           FROM public.xlm_collateral c
           JOIN public.loans l ON l.id = c.loan_id
          WHERE c.loan_id = $1 AND l.borrower_id = $2",
    )
    .bind(p.loan_id)
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| db_err(e, "collateral lookup"))?;
    let (wallet_address, required_stroops, status) =
        pinned.ok_or((StatusCode::NOT_FOUND, "No pending collateral loan found"))?;
    if status != "pending" {
        return Err((StatusCode::CONFLICT, "This loan's collateral is already settled"));
    }

    let locked_stroops =
        stellar::verify_collateral_lock(p.tx_hash.trim(), &wallet_address, &contract)
            .await
            .map_err(|m| (StatusCode::UNPROCESSABLE_ENTITY, m))?;
    if locked_stroops < required_stroops {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "The locked amount is below the required 120% collateral",
        ));
    }

    let mut tx = pool.begin().await.map_err(|e| db_err(e, "begin confirm"))?;

    // Now the locks, and a re-check of everything the unlocked read saw —
    // the state may have moved while we talked to Horizon.
    let loan: Option<(Uuid, i64, i32, i16, String)> = sqlx::query_as(
        "SELECT borrower_id, principal, rate_bps, term_months, status
           FROM public.loans WHERE id = $1 FOR UPDATE",
    )
    .bind(p.loan_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| db_err(e, "lock loan"))?;
    let (borrower_id, principal, rate_bps, term_months, loan_status) =
        loan.ok_or((StatusCode::NOT_FOUND, "No pending collateral loan found"))?;
    if borrower_id != user_id || loan_status != "pending" {
        return Err((StatusCode::CONFLICT, "This loan is not awaiting collateral"));
    }

    // One tx hash credits one position, ever (unique index) — a replayed
    // confirm, or the same lock pointed at two loans, stops here.
    let updated = sqlx::query(
        "UPDATE public.xlm_collateral
            SET locked_stroops = $1, lock_tx_hash = $2, status = 'locked', updated_at = $3
          WHERE loan_id = $4 AND status = 'pending'",
    )
    .bind(locked_stroops)
    .bind(p.tx_hash.trim())
    .bind(Utc::now().timestamp())
    .bind(p.loan_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        if e.as_database_error().is_some_and(|d| d.is_unique_violation()) {
            return (StatusCode::CONFLICT, "That lock transaction was already used");
        }
        db_err(e, "confirm collateral")
    })?;
    if updated.rows_affected() == 0 {
        return Err((StatusCode::CONFLICT, "This loan's collateral is already settled"));
    }

    commit_event(
        &mut tx,
        EventDraft {
            kind: "collateral_locked",
            user_id: Some(user_id),
            loan_id: Some(p.loan_id),
            deposit_id: None,
            // The chain reference is the rail_ref: on-chain money-in is
            // idempotent by schema exactly like PayPal captures.
            rail_ref: Some(format!("stellar:{}", p.tx_hash.trim())),
            payload: serde_json::json!({
                "stroops": locked_stroops, "required": required_stroops,
                "wallet": wallet_address
            }),
            actor_id: Some(user_id),
        },
        &[],
    )
    .await
    .map_err(|e| ledger_err(e, "collateral_locked"))?;

    disburse(&mut tx, p.loan_id, user_id, principal, rate_bps, term_months).await?;

    tx.commit().await.map_err(|e| db_err(e, "commit confirm"))?;
    tracing::info!(%user_id, loan_id = %p.loan_id, locked_stroops, "collateral confirmed, loan disbursed");

    Ok(Json(ConfirmResponse {
        loan_id: p.loan_id,
        status: "active",
        locked_stroops,
        message: "Collateral verified on-chain — your loan has been disbursed",
    }))
}
