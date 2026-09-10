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
use super::pricing;
use super::shared::db_err;
use crate::api::users::shared::{E, require_verified_user};

/// An `xlm_collateral` row in SELECT order: wallet_address, required_stroops,
/// principal_centavos, locked_stroops, status, the three pinned price legs with
/// their timestamp, and the ratio the position was struck at.
type PositionRow = (
    String,
    i64,
    // principal_centavos — what the coins stand behind (034)
    i64,
    i64,
    String,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    i32,
);

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
    /// The principal these coins stand behind — the whole loan on an
    /// `xlm_collateral` product, the borrower's coin share on a guarantor loan.
    /// The vault measures its ratio against THIS, so a resumed lock submits it
    /// rather than `loan.principal`; submitting the principal is what made the
    /// vault refuse every guarantor coin leg (034).
    pub principal_centavos: i64,
    pub locked_stroops: i64,
    pub status: String,
    /// Collateral value as % of outstanding principal at the live rate.
    pub health_pct: Option<i64>,
    /// True when health has fallen below the policy's liquidation threshold.
    pub liquidatable: bool,
    /// The rate `required_stroops` was struck at, and when the feeds behind
    /// it were read. Pinned at issuance and never recomputed — this is what
    /// lets a borrower (or a reviewer) check the conversion they were given
    /// rather than take it on trust. Null on positions predating migration 025.
    pub priced_centavos_per_xlm: Option<i64>,
    pub priced_at: Option<i64>,
    /// The same pinned quote in the shape the vault contract takes it: the
    /// XLM/USD leg it measures against Reflector (scaled 1e8), the USD/PHP
    /// leg the peso rate was crossed through (centavos), and the ratio it
    /// enforces. A borrower resuming an interrupted lock submits THESE, not a
    /// fresh number — the position is priced once, at issuance.
    pub priced_usd_per_xlm_e8: Option<i64>,
    pub priced_usd_php_centavos: Option<i64>,
    pub collateral_ratio_bps: Option<i32>,
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
    /// What a `reconciling` loan still needs before it can be marked settled
    /// (033), in centavos; 0 for every other status.
    ///
    /// Served by the engine rather than summed in the browser, and that is not
    /// just house style — it cannot be derived from anything else in this
    /// response. It is the money guarantors and the reserve are still short
    /// after the default, which lives in `loan_recoveries`. Adding up the
    /// unpaid schedule would bill the borrower for months that were never due
    /// and for their own seized deposit a second time.
    pub arrears: i64,
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

    // Keyed off the position rather than the product: a guarantor loan may
    // carry part of the borrower's own half in coins since the 50% rule, and
    // that position belongs on the loan view exactly like a pure collateral
    // one. The query already returns None when there is no position, so the
    // product test only ever hid rows that exist.
    let collateral = {
        let row: Option<PositionRow> = sqlx::query_as(
            "SELECT wallet_address, required_stroops, principal_centavos, locked_stroops, status,
                    priced_centavos_per_xlm, priced_at,
                    priced_usd_per_xlm_e8, priced_usd_php_centavos, collateral_ratio_bps
               FROM public.xlm_collateral WHERE loan_id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| db_err(e, "collateral"))?;
        row.map(|(wallet_address, required_stroops, principal_centavos, locked_stroops, c_status, priced_centavos_per_xlm, priced_at, priced_usd_per_xlm_e8, priced_usd_php_centavos, collateral_ratio_bps)| {
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
                principal_centavos,
                locked_stroops,
                status: c_status,
                health_pct,
                liquidatable,
                priced_centavos_per_xlm,
                priced_at,
                priced_usd_per_xlm_e8,
                priced_usd_php_centavos,
                collateral_ratio_bps: Some(collateral_ratio_bps),
            }
        })
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

    // Only a loan being settled has arrears; skipping the query for every
    // other status keeps the common list read at the number of queries it
    // already made.
    let arrears: i64 = if status == "reconciling" {
        sqlx::query_scalar(
            "SELECT COALESCE(SUM(amount - refunded), 0)::BIGINT
               FROM public.loan_recoveries
              WHERE loan_id = $1 AND source IN ('guarantor_deposit', 'reserve_fund')",
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|e| db_err(e, "loan arrears"))?
    } else {
        0
    };

    Ok(LoanView {
        id,
        product,
        principal,
        rate_bps,
        term_months,
        status,
        principal_outstanding: outstanding,
        arrears,
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
    let fx = pricing::for_display(&pool).await?.centavos_per_xlm;

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
    let fx = pricing::for_display(&pool).await?.centavos_per_xlm;

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
