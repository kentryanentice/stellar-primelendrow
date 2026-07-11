use axum::{Extension, Json, http::HeaderMap};
use chrono::Utc;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::shared::{CHALLENGE_TTL_SECS, challenge_message};
use crate::api::users::shared::{E, require_verified_user};

#[derive(Serialize)]
pub struct ChallengeResponse {
    pub nonce: String,
    pub message: String,
    pub expires_at: i64,
}

/// Issues a one-time, short-lived nonce for the caller to sign with the
/// wallet they want to connect. Redeemed (deleted, single use) by
/// api::wallets::connect, which re-derives the exact message from the
/// stored nonce + expiry rather than trusting anything the client echoes
/// back.
pub async fn challenge(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
) -> Result<Json<ChallengeResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    let nonce = Uuid::new_v4().to_string();
    let expires_at = Utc::now().timestamp() + CHALLENGE_TTL_SECS;

    sqlx::query(
        "INSERT INTO public.wallet_challenges (nonce, user_id, expires_at) VALUES ($1, $2, $3)",
    )
    .bind(&nonce)
    .bind(user_id)
    .bind(expires_at)
    .execute(&pool)
    .await
    .map_err(|e| {
        tracing::error!("DB wallet challenge insert: {e}");
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Unable to start wallet verification",
        )
    })?;

    Ok(Json(ChallengeResponse {
        message: challenge_message(&nonce, expires_at),
        nonce,
        expires_at,
    }))
}
