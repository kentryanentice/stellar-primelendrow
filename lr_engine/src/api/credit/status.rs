use axum::{Extension, Json, http::HeaderMap};
use serde::Serialize;
use sqlx::PgPool;

use crate::api::users::shared::{E, require_verified_user};

#[derive(Serialize)]
pub struct CreditScoreResponse {
    pub score: i16,
    pub updated_at: i64,
}

/// The owner's own credit score. Read-only for now — nothing in the codebase
/// yet computes or adjusts it; every account starts at 50 via a DB trigger
/// (migration 015) and this just surfaces the current value.
///
/// require_verified_user (not require_user): a score is meaningless for an
/// account still gated behind KYC (Pending/Verifying), so this is enforced
/// here — not just skipped client-side — a Pending/Verifying account hitting
/// this directly gets a 403, same as any other member-only endpoint.
pub async fn status(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
) -> Result<Json<CreditScoreResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    let row: Option<(i16, i64)> = sqlx::query_as(
        "SELECT score, updated_at FROM public.credit_scores WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        tracing::error!("DB credit score: {e}");
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Unable to load credit score",
        )
    })?;

    // every user gets a row via the DB trigger on insert (migration 015); a
    // missing row would only mean an account predating it slipped the
    // backfill — surface the same default rather than erroring the page
    let (score, updated_at) = row.unwrap_or((50, 0));

    Ok(Json(CreditScoreResponse { score, updated_at }))
}
