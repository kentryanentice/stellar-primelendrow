use axum::{Extension, Json, http::HeaderMap};
use serde::Serialize;
use sqlx::PgPool;

use crate::api::users::shared::{E, require_user};

#[derive(Serialize)]
pub struct KycStatusResponse {
    /// "none" | "verifying" | "approved" | "rejected"
    pub status: String,
    pub submitted_at: Option<i64>,
    pub reviewed_at: Option<i64>,
    /// Only ever present on the owner's own rejected submission.
    pub rejection_reason: Option<String>,
}

/// The owner's view of their latest submission. Deliberately returns no PII —
/// the client already knows what it typed, and echoing decrypted fields would
/// turn a stolen session cookie into a document leak.
pub async fn status(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
) -> Result<Json<KycStatusResponse>, E> {
    let user_id = require_user(&pool, &headers).await?;

    let row: Option<(String, i64, Option<i64>, Option<String>)> = sqlx::query_as(
        "SELECT status, created_at, reviewed_at, rejection_reason
           FROM public.kyc_submissions
          WHERE user_id = $1
          ORDER BY created_at DESC
          LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        tracing::error!("DB kyc status: {e}");
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Unable to load status",
        )
    })?;

    Ok(Json(match row {
        None => KycStatusResponse {
            status: "none".into(),
            submitted_at: None,
            reviewed_at: None,
            rejection_reason: None,
        },
        Some((status, submitted_at, reviewed_at, rejection_reason)) => {
            let rejection_reason = (status == "rejected").then_some(rejection_reason).flatten();
            KycStatusResponse {
                status,
                submitted_at: Some(submitted_at),
                reviewed_at,
                rejection_reason,
            }
        }
    }))
}
