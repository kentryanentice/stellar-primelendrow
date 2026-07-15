//! POST /loans/payments — the caller's repayment history, paginated, plus
//! all-time totals.
//!
//! One row per captured PayPal payment (loan_payments, migration 023), joined
//! to the loan for its product label. Read-only; every number is the split the
//! engine recorded at repayment time. The Pay page's "Repaid to date" hero
//! reads `totals` (summed across every payment, not just the page on screen)
//! and "Payment history" reads `items` (one page at a time) — same split as
//! GET /pool (badge totals) vs POST /pool/deposits (paginated lots), just
//! bundled into one response since both live on the same page here.

use axum::{Extension, Json, http::HeaderMap};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::shared::db_err;
use crate::api::users::shared::{E, require_verified_user};

fn default_page() -> i64 {
    1
}
/// Fixed server-side, same rationale as deposits_list::PAGE_SIZE.
const PAGE_SIZE: i64 = 6;

#[derive(Deserialize)]
pub struct PaymentsRequest {
    #[serde(default = "default_page")]
    page: i64,
}

#[derive(Serialize)]
pub struct PaymentView {
    pub id: i64,
    pub loan_id: Uuid,
    pub product: String,
    pub amount_received: i64,
    pub interest_paid: i64,
    pub principal_paid: i64,
    pub excess: i64,
    pub paid_at: i64,
}

#[derive(Serialize)]
pub struct PaymentTotals {
    pub amount_received: i64,
    pub interest_paid: i64,
    pub principal_paid: i64,
}

#[derive(Serialize)]
pub struct PaymentsResponse {
    pub items: Vec<PaymentView>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
    pub totals: PaymentTotals,
}

pub async fn list(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(q): Json<PaymentsRequest>,
) -> Result<Json<PaymentsResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    let page = q.page.max(1);
    let offset = (page - 1) * PAGE_SIZE;

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM public.loan_payments WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| db_err(e, "payments count"))?;

    // All-time totals — a separate aggregate query rather than summing the
    // page in hand, since the page is only ever a slice of the full history.
    let (amount_received, interest_paid, principal_paid): (i64, i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(amount_received), 0)::BIGINT,
                COALESCE(SUM(interest_paid), 0)::BIGINT,
                COALESCE(SUM(principal_paid), 0)::BIGINT
           FROM public.loan_payments
          WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| db_err(e, "payment totals"))?;

    let rows: Vec<(i64, Uuid, String, i64, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT p.id, p.loan_id, l.product,
                p.amount_received, p.interest_paid, p.principal_paid, p.excess, p.paid_at
           FROM public.loan_payments p
           JOIN public.loans l ON l.id = p.loan_id
          WHERE p.user_id = $1
          ORDER BY p.paid_at DESC, p.id DESC
          LIMIT $2 OFFSET $3",
    )
    .bind(user_id)
    .bind(PAGE_SIZE)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "payments page"))?;

    let items = rows
        .into_iter()
        .map(|(id, loan_id, product, amount_received, interest_paid, principal_paid, excess, paid_at)| PaymentView {
            id, loan_id, product, amount_received, interest_paid, principal_paid, excess, paid_at,
        })
        .collect();

    let total_pages = if total == 0 { 1 } else { (total + PAGE_SIZE - 1) / PAGE_SIZE };

    Ok(Json(PaymentsResponse {
        items,
        total,
        page,
        page_size: PAGE_SIZE,
        total_pages,
        totals: PaymentTotals { amount_received, interest_paid, principal_paid },
    }))
}
