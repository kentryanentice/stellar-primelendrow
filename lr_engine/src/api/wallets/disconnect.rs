use axum::{
    Extension, Json,
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::shared::audit;
use crate::api::users::shared::{E, MessageResponse, require_verified_user};

#[derive(Deserialize)]
pub struct DisconnectInput {
    wallet_id: Uuid,
}

/// Flips a wallet to "disconnected" — never a DELETE, and never touches
/// kyc_submissions no matter which wallet this is, including the
/// KYC-anchor one. That row's wallet_address is the permanent record of
/// what was reviewed at verification time; this only ever changes what's in
/// the live `wallets` table.
pub async fn disconnect(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(p): Json<DisconnectInput>,
) -> Result<Json<MessageResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;
    let now = Utc::now().timestamp();

    // ownership check lives in the WHERE clause — wallet_id alone is never
    // trusted
    let address: Option<String> = sqlx::query_scalar(
        "UPDATE public.wallets
            SET status = 'disconnected', disconnected_at = $1, updated_at = $1
          WHERE id = $2 AND user_id = $3 AND status = 'active'
          RETURNING address",
    )
    .bind(now)
    .bind(p.wallet_id)
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        tracing::error!("DB wallet disconnect: {e}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Unable to disconnect wallet",
        )
    })?;

    let address = address.ok_or((StatusCode::NOT_FOUND, "Wallet not found"))?;

    audit(&pool, p.wallet_id, user_id, &address, "disconnected").await;
    tracing::info!(%user_id, wallet_id = %p.wallet_id, "wallet disconnected");

    Ok(Json(MessageResponse {
        message: "Wallet disconnected",
    }))
}
