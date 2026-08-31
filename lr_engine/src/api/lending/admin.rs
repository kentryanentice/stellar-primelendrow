//! Admin lending controls. For now: the manually-filed XLM/PHP rate.
//!
//! This is no longer how loans get priced. Since migration 025 the live rate
//! comes from `pricing`, which agrees several independent public feeds and
//! REFUSES to price a loan when they don't — an admin cannot talk the engine
//! into issuing at a number of their choosing, which is the whole point of
//! taking the rate off a form. What this endpoint still does is seed the
//! fallback that screens fall back to when every feed is unreachable, and
//! give the operator a way to file a rate on a fresh deployment.
//!
//! Append-only history (fx_rates), and the change itself is an event (D8), so
//! a filed rate is always attributable to the admin who filed it.

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

    sqlx::query(
        "INSERT INTO public.fx_rates (centavos_per_xlm, actor_id, source, method)
         VALUES ($1, $2, 'admin', 'filed by an administrator')",
    )
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
        message: "Fallback XLM/PHP rate filed — live loans are still priced from the public feeds",
    }))
}
