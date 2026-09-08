//! GET /stripe/connect  — start (or resume) Express onboarding
//! GET /stripe/return   — where Stripe sends the member back when they finish
//! GET /stripe/refresh  — where Stripe sends them when the link went stale
//!
//! The mirror of `api::paypal::connect`, and the differences are Stripe's, not
//! ours.
//!
//! PayPal's flow is an OAuth authorization: the member consents and PayPal
//! hands back a code that resolves to an account they already had. Stripe has
//! no such account to point at — money can only be sent to a *connected
//! account* — so the engine creates one first and then sends the member
//! through Stripe's onboarding to fill it in. The row therefore exists before
//! the member has finished, which is why `details_submitted` and
//! `payouts_enabled` are stored: an `acct_…` on its own is not yet somewhere
//! money can go.
//!
//! The `state` token carries the whole trust of the return leg, so it is a
//! random 256-bit value, stored server-side against the member who started the
//! flow, and expired after thirty minutes. The callback identifies the member
//! from that row and **not** from a session cookie: the request is a
//! cross-site top-level redirect from stripe.com, and a flow that only works
//! when the browser chooses to send cookies is a flow that breaks quietly.
//!
//! One difference from the PayPal state: onboarding can legitimately bounce
//! through `refresh_url` several times before it completes, so the token is
//! consumed only on the final return — a refresh reads it without claiming it.

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
use crate::infra::stripe;

/// Longer than the PayPal equivalent's ten minutes: Stripe onboarding asks for
/// identity details and bank information, which is not a ten-minute job on a
/// phone.
const STATE_TTL_SECS: i64 = 1800;

#[derive(Serialize)]
pub struct StartResponse {
    /// Where to send the member. The frontend navigates the top-level window
    /// here — Stripe refuses to be framed.
    pub url: String,
}

pub async fn start(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
) -> Result<Json<StartResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    if !stripe::can_connect() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Stripe isn't configured on this deployment yet",
        ));
    }

    // Reuse the member's existing connected account when there is one: a
    // second `acct_…` for the same person would strand whatever onboarding
    // they already completed, and the unique index would refuse it anyway.
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT account_id FROM public.stripe_accounts WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| db_err(e, "stripe account lookup"))?;

    let account_id = match existing {
        Some(id) => id,
        None => {
            let email: String =
                sqlx::query_scalar("SELECT email FROM public.users WHERE id = $1")
                    .bind(user_id)
                    .fetch_one(&pool)
                    .await
                    .map_err(|e| db_err(e, "member email"))?;

            let account_id = stripe::create_express_account(&email)
                .await
                .map_err(|m| (StatusCode::SERVICE_UNAVAILABLE, m))?;

            // Written before the member is sent anywhere. If they abandon
            // onboarding the row simply stays incomplete — far better than
            // creating an account at Stripe that this engine has no record of.
            sqlx::query(
                "INSERT INTO public.stripe_accounts (user_id, account_id, email, status, updated_at)
                 VALUES ($1, $2, $3, 'active', $4)
                 ON CONFLICT (user_id) DO UPDATE
                    SET account_id = EXCLUDED.account_id,
                        email = EXCLUDED.email,
                        status = 'active',
                        updated_at = EXCLUDED.updated_at",
            )
            .bind(user_id)
            .bind(&account_id)
            .bind(&email)
            .bind(Utc::now().timestamp())
            .execute(&pool)
            .await
            .map_err(|e| db_err(e, "stripe account insert"))?;

            account_id
        }
    };

    let state = hex::encode(rand::random::<[u8; 32]>());
    sqlx::query(
        "INSERT INTO public.stripe_connect_states (state, user_id, expires_at)
         VALUES ($1, $2, $3)",
    )
    .bind(&state)
    .bind(user_id)
    .bind(Utc::now().timestamp() + STATE_TTL_SECS)
    .execute(&pool)
    .await
    .map_err(|e| db_err(e, "stripe connect state"))?;

    let url = stripe::onboarding_link(&account_id, &state)
        .await
        .map_err(|m| (StatusCode::SERVICE_UNAVAILABLE, m))?;

    Ok(Json(StartResponse { url }))
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    #[serde(default)]
    state: Option<String>,
}

/// Everything here ends in a redirect back to the app, never a JSON error:
/// the member is looking at a browser window, and a raw 4xx body is a dead
/// end. The reason travels as a query flag the Settings page renders.
pub async fn callback(
    Extension(pool): Extension<PgPool>,
    Query(q): Query<CallbackQuery>,
) -> impl IntoResponse {
    match finish_onboarding(&pool, q).await {
        Ok(true) => Redirect::to(&app_url("stripe=connected")),
        // Stripe returned them, but the account still can't receive money —
        // usually more information is needed. Saying so is the whole point:
        // "connected" would be a lie they'd only discover at withdrawal.
        Ok(false) => Redirect::to(&app_url("stripe=error&reason=incomplete")),
        Err(reason) => {
            tracing::warn!(reason, "stripe connect failed");
            Redirect::to(&app_url(&format!("stripe=error&reason={reason}")))
        }
    }
}

/// Stripe sends the member here when the onboarding link expired before they
/// finished. A fresh link needs an authenticated call, which this redirect is
/// not, so it bounces them back to Settings with a flag that tells the card to
/// offer the button again. The state row is deliberately left unclaimed.
pub async fn refresh(Query(q): Query<CallbackQuery>) -> impl IntoResponse {
    let _ = q;
    Redirect::to(&app_url("stripe=error&reason=expired"))
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

/// Returns whether the account came back able to receive money.
async fn finish_onboarding(pool: &PgPool, q: CallbackQuery) -> Result<bool, &'static str> {
    let state = q.state.ok_or("nostate")?;

    // Single-use by construction: the DELETE is the claim. A replayed return
    // finds nothing and stops here.
    let claimed: Option<(Uuid, i64)> = sqlx::query_as(
        "DELETE FROM public.stripe_connect_states WHERE state = $1
         RETURNING user_id, expires_at",
    )
    .bind(&state)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("DB stripe state claim: {e}");
        "server"
    })?;
    let (user_id, expires_at) = claimed.ok_or("badstate")?;
    if expires_at <= Utc::now().timestamp() {
        return Err("expired");
    }

    let account_id: Option<String> = sqlx::query_scalar(
        "SELECT account_id FROM public.stripe_accounts WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("DB stripe account read: {e}");
        "server"
    })?;
    let account_id = account_id.ok_or("noaccount")?;

    // Stripe is asked, not assumed. The member reaching the return URL means
    // they walked to the end of the form, not that Stripe accepted them.
    let account = stripe::account_status(&account_id).await.map_err(|m| {
        tracing::error!("stripe account status: {m}");
        "stripe"
    })?;

    sqlx::query(
        "UPDATE public.stripe_accounts
            SET email = CASE WHEN $1 = '' THEN email ELSE $1 END,
                payouts_enabled = $2,
                details_submitted = $3,
                status = 'active',
                updated_at = $4
          WHERE user_id = $5",
    )
    .bind(&account.email)
    .bind(account.payouts_enabled)
    .bind(account.details_submitted)
    .bind(Utc::now().timestamp())
    .bind(user_id)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("DB stripe account update: {e}");
        "server"
    })?;

    tracing::info!(%user_id, payouts_enabled = account.payouts_enabled, "stripe account onboarded");
    Ok(account.payouts_enabled)
}
