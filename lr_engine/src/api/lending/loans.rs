//! GET /loans and POST /loans/history — the caller's own loans with their
//! pinned schedules, collateral state, and guarantor roster. Read-only; every
//! number is the engine's, including the XLM health ratio the liquidation
//! rule watches.
//!
//! Two shapes over the same per-loan detail (`build_loan_view`):
//!   list     unpaginated — Pay.tsx's "find my one open loan" (the DB's
//!            one-open-loan-per-borrower index means this is at most 1 row
//!            of real interest; the rest is cheap history).
//!   history  paginated — the Borrow page's "Your loans" card, since a
//!            long-standing borrower's closed-loan history only grows.

use axum::{Extension, Json, http::HeaderMap};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::domain;
use super::policy::{self, Policy};
use super::shared::db_err;
use crate::api::users::shared::{E, require_verified_user};

#[derive(Serialize)]
pub struct ScheduleView {
    pub installment: i16,
    pub due_at: i64,
    pub principal_due: i64,
    pub interest_due: i64,
    pub principal_paid: i64,
    pub interest_paid: i64,
    pub status: String,
}

#[derive(Serialize)]
pub struct CollateralView {
    pub wallet_address: String,
    pub required_stroops: i64,
    pub locked_stroops: i64,
    pub status: String,
    /// Collateral value as % of outstanding principal at the live rate.
    pub health_pct: Option<i64>,
    /// True when health has fallen below the policy's liquidation threshold.
    pub liquidatable: bool,
}

#[derive(Serialize)]
pub struct GuarantorView {
    pub username: String,
    pub pledge_amount: i64,
    pub status: String,
}

#[derive(Serialize)]
pub struct LoanView {
    pub id: Uuid,
    pub product: String,
    pub principal: i64,
    pub rate_bps: i32,
    pub term_months: i16,
    pub status: String,
    pub principal_outstanding: i64,
    pub disbursed_at: Option<i64>,
    pub closed_at: Option<i64>,
    pub created_at: i64,
    pub schedule: Vec<ScheduleView>,
    pub collateral: Option<CollateralView>,
    pub guarantors: Vec<GuarantorView>,
}

#[derive(Serialize)]
pub struct LoansResponse {
    pub loans: Vec<LoanView>,
}

type LoanRow = (Uuid, String, i64, i32, i16, String, i64, Option<i64>, Option<i64>, i64);

/// Fetches one loan's schedule/collateral/guarantors and assembles the view.
/// Shared by `list` and `history` so the two endpoints can't drift on what a
/// "loan" looks like.
async fn build_loan_view(pool: &PgPool, rules: &Policy, fx: i64, row: LoanRow) -> Result<LoanView, E> {
    let (id, product, principal, rate_bps, term_months, status, outstanding, disbursed_at, closed_at, created_at) = row;

    let schedule_rows: Vec<(i16, i64, i64, i64, i64, i64, String)> = sqlx::query_as(
        "SELECT installment, due_at, principal_due, interest_due,
                principal_paid, interest_paid, status
           FROM public.loan_schedule
          WHERE loan_id = $1
          ORDER BY installment",
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(|e| db_err(e, "schedule"))?;

    let collateral = if product == "xlm_collateral" {
        let row: Option<(String, i64, i64, String)> = sqlx::query_as(
            "SELECT wallet_address, required_stroops, locked_stroops, status
               FROM public.xlm_collateral WHERE loan_id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| db_err(e, "collateral"))?;
        row.map(|(wallet_address, required_stroops, locked_stroops, c_status)| {
            // Health = collateral value / outstanding, at the live rate.
            // Display + liquidation watch; the seize decision itself is an
            // admin action against the vault, never automatic here.
            let health_pct = if outstanding > 0 && locked_stroops > 0 {
                Some(domain::collateral_value_centavos(locked_stroops, fx) * 100 / outstanding)
            } else {
                None
            };
            let liquidatable = c_status == "locked"
                && health_pct.is_some_and(|h| h < rules.params.xlm_liquidation_pct);
            CollateralView {
                wallet_address,
                required_stroops,
                locked_stroops,
                status: c_status,
                health_pct,
                liquidatable,
            }
        })
    } else {
        None
    };

    let guarantors: Vec<(String, i64, String)> = sqlx::query_as(
        "SELECT u.username, g.pledge_amount, g.status
           FROM public.loan_guarantors g
           JOIN public.users u ON u.id = g.guarantor_id
          WHERE g.loan_id = $1
          ORDER BY g.created_at",
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(|e| db_err(e, "guarantors"))?;

    Ok(LoanView {
        id,
        product,
        principal,
        rate_bps,
        term_months,
        status,
        principal_outstanding: outstanding,
        disbursed_at,
        closed_at,
        created_at,
        schedule: schedule_rows
            .into_iter()
            .map(|(installment, due_at, principal_due, interest_due, principal_paid, interest_paid, status)| ScheduleView {
                installment, due_at, principal_due, interest_due, principal_paid, interest_paid, status,
            })
            .collect(),
        collateral,
        guarantors: guarantors
            .into_iter()
            .map(|(username, pledge_amount, status)| GuarantorView { username, pledge_amount, status })
            .collect(),
    })
}

pub async fn list(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
) -> Result<Json<LoansResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    let rules = policy::active(&pool).await?;
    let fx = policy::fx_centavos_per_xlm(&pool).await?;

    let loan_rows: Vec<LoanRow> = sqlx::query_as(
        "SELECT id, product, principal, rate_bps, term_months, status,
                principal_outstanding, disbursed_at, closed_at, created_at
           FROM public.loans
          WHERE borrower_id = $1
          ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "loans"))?;

    let mut loans = Vec::with_capacity(loan_rows.len());
    for row in loan_rows {
        loans.push(build_loan_view(&pool, &rules, fx, row).await?);
    }

    Ok(Json(LoansResponse { loans }))
}

// ---- history: paginated, for the Borrow page's "Your loans" card ----

fn default_page() -> i64 {
    1
}
/// Fixed server-side, same rationale as deposits_list::PAGE_SIZE.
const PAGE_SIZE: i64 = 6;

#[derive(Deserialize)]
pub struct HistoryRequest {
    #[serde(default = "default_page")]
    page: i64,
}

#[derive(Serialize)]
pub struct LoansHistoryResponse {
    pub items: Vec<LoanView>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}

pub async fn history(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(q): Json<HistoryRequest>,
) -> Result<Json<LoansHistoryResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    let rules = policy::active(&pool).await?;
    let fx = policy::fx_centavos_per_xlm(&pool).await?;

    let page = q.page.max(1);
    let offset = (page - 1) * PAGE_SIZE;

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM public.loans WHERE borrower_id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| db_err(e, "loans count"))?;

    let loan_rows: Vec<LoanRow> = sqlx::query_as(
        "SELECT id, product, principal, rate_bps, term_months, status,
                principal_outstanding, disbursed_at, closed_at, created_at
           FROM public.loans
          WHERE borrower_id = $1
          ORDER BY created_at DESC, id
          LIMIT $2 OFFSET $3",
    )
    .bind(user_id)
    .bind(PAGE_SIZE)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "loans page"))?;

    let mut items = Vec::with_capacity(loan_rows.len());
    for row in loan_rows {
        items.push(build_loan_view(&pool, &rules, fx, row).await?);
    }

    let total_pages = if total == 0 { 1 } else { (total + PAGE_SIZE - 1) / PAGE_SIZE };

    Ok(Json(LoansHistoryResponse {
        items,
        total,
        page,
        page_size: PAGE_SIZE,
        total_pages,
    }))
}
