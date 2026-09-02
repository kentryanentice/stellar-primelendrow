//! GET /paypal/connect  — start "Log in with PayPal"
//! GET /paypal/callback — where PayPal sends the member back
//!
//! The `state` token carries the whole trust of this flow, so it is a random
//! 256-bit value, stored server-side against the member who started the flow,
//! usable once, and expired after ten minutes. The callback identifies the
//! member from that row and **not** from a session cookie: the request is a
//! cross-site top-level redirect from paypal.com, and a flow that only works
//! when the browser chooses to send cookies is a flow that breaks quietly.

use axum::{
    Extension, Json,
    extract::Query,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::lending::shared::db_err;
use crate::api::users::shared::{E, require_verified_user};
use crate::infra::paypal;

/// Long enough to be unguessable, short enough to live in a URL.
const STATE_TTL_SECS: i64 = 600;

#[derive(Serialize)]
pub struct StartResponse {
    /// Where to send the member. The frontend navigates the top-level window
    /// here — PayPal refuses to be framed.
    pub url: String,
}

pub async fn start(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
) -> Result<Json<StartResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    let state = hex::encode(rand::random::<[u8; 32]>());
    let url = paypal::connect_url(&state).ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "PayPal isn't configured on this deployment yet",
    ))?;

    sqlx::query(
        "INSERT INTO public.paypal_connect_states (state, user_id, expires_at)
         VALUES ($1, $2, $3)",
    )
    .bind(&state)
    .bind(user_id)
    .bind(Utc::now().timestamp() + STATE_TTL_SECS)
    .execute(&pool)
    .await
    .map_err(|e| db_err(e, "paypal connect state"))?;

    Ok(Json(StartResponse { url }))
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    state: Option<String>,
    /// PayPal sends this when the member declines.
    #[serde(default)]
    error: Option<String>,
}

/// Everything here ends in a redirect back to the app, never a JSON error:
/// the member is looking at a browser window, and a raw 4xx body is a dead
/// end. The reason travels as a query flag the Settings page renders.
pub async fn callback(
    Extension(pool): Extension<PgPool>,
    Query(q): Query<CallbackQuery>,
) -> impl IntoResponse {
    match link_account(&pool, q).await {
        Ok(()) => Redirect::to(&app_url("paypal=connected")),
        Err(reason) => {
            tracing::warn!(reason, "paypal connect failed");
            Redirect::to(&app_url(&format!("paypal=error&reason={reason}")))
        }
    }
}

/// Back to the Settings page, with a flag saying how it went.
///
/// CLIENT_URL is a comma-separated *list* — the CORS layer allows every entry
/// (see engine.rs). A redirect can only go to one place, so the first entry is
/// the canonical app origin; splitting here is what keeps the whole list from
/// being pasted into a Location header.
fn app_url(flag: &str) -> String {
    let raw = std::env::var("CLIENT_URL").unwrap_or_default();
    let base = raw
        .split(',')
        .map(str::trim)
        .find(|s| !s.is_empty())
        .unwrap_or("http://localhost:5173");
    format!("{}/settings?{flag}", base.trim_end_matches('/'))
}

async fn link_account(pool: &PgPool, q: CallbackQuery) -> Result<(), &'static str> {
    if q.error.is_some() {
        return Err("declined");
    }
    let code = q.code.ok_or("nocode")?;
    let state = q.state.ok_or("nostate")?;

    // Single-use by construction: the DELETE is the claim. A replayed
    // callback finds nothing and stops here.
    let claimed: Option<(Uuid, i64)> = sqlx::query_as(
        "DELETE FROM public.paypal_connect_states WHERE state = $1
         RETURNING user_id, expires_at",
    )
    .bind(&state)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("DB paypal state claim: {e}");
        "server"
    })?;
    let (user_id, expires_at) = claimed.ok_or("badstate")?;
    if expires_at <= Utc::now().timestamp() {
        return Err("expired");
    }

    let account = paypal::exchange_code(code.trim()).await.map_err(|m| {
        tracing::error!("paypal exchange: {m}");
        "paypal"
    })?;

    // Relinking replaces the destination; the unique index on payer_id
    // refuses an account already linked to somebody else, which is the same
    // one-account-one-member rule KYC applies to government IDs.
    sqlx::query(
        "INSERT INTO public.paypal_accounts
            (user_id, payer_id, email, verified, status, updated_at)
         VALUES ($1, $2, $3, $4, 'active', $5)
         ON CONFLICT (user_id) DO UPDATE
            SET payer_id = EXCLUDED.payer_id,
                email = EXCLUDED.email,
                verified = EXCLUDED.verified,
                status = 'active',
                updated_at = EXCLUDED.updated_at",
    )
    .bind(user_id)
    .bind(&account.payer_id)
    .bind(&account.email)
    .bind(account.verified)
    .bind(Utc::now().timestamp())
    .execute(pool)
    .await
    .map_err(|e| {
        if e.as_database_error().is_some_and(|d| d.is_unique_violation()) {
            return "taken";
        }
        tracing::error!("DB paypal account upsert: {e}");
        "server"
    })?;

    tracing::info!(%user_id, "paypal account connected");
    Ok(())
}
