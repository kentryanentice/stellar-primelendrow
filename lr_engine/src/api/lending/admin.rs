//! Admin lending controls. For now: the XLM/PHP rate the 120%/110% collateral
//! rules value against. Append-only history (fx_rates), and the change itself
//! is an event (D8) — a rate move that flips someone to liquidatable is
//! always attributable.

use axum::{Extension, Json, http::{HeaderMap, StatusCode}};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use super::ledger::{EventDraft, commit_event};
use super::shared::{db_err, ledger_err};
use crate::api::users::shared::{E, require_admin};

#[derive(Deserialize)]
pub struct FxRateInput {
    /// Whole centavos one XLM is worth (₱18.00 -> 1800).
    centavos_per_xlm: i64,
}

#[derive(Serialize)]
pub struct FxRateResponse {
    pub centavos_per_xlm: i64,
    pub message: &'static str,
}

pub async fn set_fx_rate(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(p): Json<FxRateInput>,
) -> Result<Json<FxRateResponse>, E> {
    let admin_id = require_admin(&pool, &headers).await?;
    if p.centavos_per_xlm <= 0 || p.centavos_per_xlm > 100_000_000 {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Invalid rate"));
    }

    let mut tx = pool.begin().await.map_err(|e| db_err(e, "begin fx"))?;

    sqlx::query("INSERT INTO public.fx_rates (centavos_per_xlm, actor_id) VALUES ($1, $2)")
        .bind(p.centavos_per_xlm)
        .bind(admin_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| db_err(e, "insert fx"))?;

    commit_event(
        &mut tx,
        EventDraft {
            kind: "fx_rate_set",
            user_id: None,
            loan_id: None,
            deposit_id: None,
            rail_ref: None,
            payload: serde_json::json!({ "centavos_per_xlm": p.centavos_per_xlm }),
            actor_id: Some(admin_id),
        },
        &[],
    )
    .await
    .map_err(|e| ledger_err(e, "fx_rate_set"))?;

    tx.commit().await.map_err(|e| db_err(e, "commit fx"))?;
    tracing::info!(%admin_id, rate = p.centavos_per_xlm, "fx rate set");

    Ok(Json(FxRateResponse {
        centavos_per_xlm: p.centavos_per_xlm,
        message: "XLM/PHP rate updated",
    }))
}
