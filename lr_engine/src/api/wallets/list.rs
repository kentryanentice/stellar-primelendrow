use axum::{Extension, Json, http::HeaderMap};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::users::shared::{E, require_verified_user};

#[derive(Serialize)]
pub struct WalletItem {
    pub id: Uuid,
    pub address: String,
    pub label: Option<String>,
    /// "kyc_verified" | "user_added"
    pub source: String,
    /// "active" | "disconnected"
    pub status: String,
    pub connected_at: i64,
    pub disconnected_at: Option<i64>,
}

#[derive(Serialize)]
pub struct WalletsResponse {
    pub wallets: Vec<WalletItem>,
}

/// The caller's full wallet history — active and disconnected alike, nothing
/// hidden, consistent with this codebase's audit-trail philosophy elsewhere
/// (kyc_audit_log, kyc_submissions rows surviving rejection).
pub async fn list(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
) -> Result<Json<WalletsResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    let rows: Vec<(Uuid, String, Option<String>, String, String, i64, Option<i64>)> = sqlx::query_as(
        "SELECT id, address, label, source, status, connected_at, disconnected_at
           FROM public.wallets
          WHERE user_id = $1
          ORDER BY (source = 'kyc_verified') DESC, connected_at ASC",
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        tracing::error!("DB wallets list: {e}");
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Unable to load wallets",
        )
    })?;

    let wallets = rows
        .into_iter()
        .map(
            |(id, address, label, source, status, connected_at, disconnected_at)| WalletItem {
                id,
                address,
                label,
                source,
                status,
                connected_at,
                disconnected_at,
            },
        )
        .collect();

    Ok(Json(WalletsResponse { wallets }))
}
