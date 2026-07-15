//! POST /pool/deposits — the caller's own deposit lots, paginated.
//!
//! GET /pool sums these by badge for the running totals; this is the "Your
//! deposits" list itself. Split out from that response so a member who has
//! accumulated many lots doesn't pull them all on every pool load. Same
//! page-in-body shape as kyc::admin::pending (POST so the page never rides a
//! client-controlled query string), fixed page size.

use axum::{Extension, Json, http::HeaderMap};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::shared::db_err;
use crate::api::users::shared::{E, require_verified_user};

fn default_page() -> i64 {
    1
}
/// Fixed server-side — the caller only ever picks *which* page, never *how
/// large*, same rationale as the KYC admin queue's PAGE_SIZE.
const PAGE_SIZE: i64 = 6;

#[derive(Deserialize)]
pub struct DepositsPageRequest {
    #[serde(default = "default_page")]
    page: i64,
}

#[derive(Serialize)]
pub struct LotView {
    pub id: Uuid,
    pub amount: i64,
    pub badge: String,
    pub backing_loan: Option<Uuid>,
    pub created_at: i64,
}

#[derive(Serialize)]
pub struct DepositsPageResponse {
    pub items: Vec<LotView>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}

pub async fn list(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(q): Json<DepositsPageRequest>,
) -> Result<Json<DepositsPageResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    // clamp rather than reject: page=0 or a stale page past the end gets a
    // sane response instead of a 4xx round-trip
    let page = q.page.max(1);
    let offset = (page - 1) * PAGE_SIZE;

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM public.deposits WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| db_err(e, "deposits count"))?;

    let rows: Vec<(Uuid, i64, String, Option<Uuid>, i64)> = sqlx::query_as(
        "SELECT id, amount, badge, backing_loan, created_at
           FROM public.deposits
          WHERE user_id = $1
          ORDER BY created_at DESC, id
          LIMIT $2 OFFSET $3",
    )
    .bind(user_id)
    .bind(PAGE_SIZE)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "deposits page"))?;

    let items = rows
        .into_iter()
        .map(|(id, amount, badge, backing_loan, created_at)| LotView { id, amount, badge, backing_loan, created_at })
        .collect();

    let total_pages = if total == 0 { 1 } else { (total + PAGE_SIZE - 1) / PAGE_SIZE };

    Ok(Json(DepositsPageResponse {
        items,
        total,
        page,
        page_size: PAGE_SIZE,
        total_pages,
    }))
}
