//! POST /lending/admin/loans — the operator's view of every loan, paginated.
//!
//! The member-facing endpoints are all scoped to the caller by construction;
//! this is the one that deliberately isn't, so it is `require_admin` and
//! nothing else. It carries what an operator needs to decide whether a loan
//! should be called: the schedule with what is actually unpaid, the collateral
//! position behind it, and — once a default has been declared — the recovery
//! waterfall as far as it has run.
//!
//! Batched rather than N+1: the page's loan ids are collected once and the
//! schedule, collateral and recovery rows are fetched with `= ANY($1)`, so a
//! page costs four queries whatever its size.

use axum::{Extension, Json, http::HeaderMap};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::shared::db_err;
use crate::api::users::shared::{E, require_admin};

fn default_page() -> i64 {
    1
}
/// Fixed server-side, same rationale as deposits_list::PAGE_SIZE.
const PAGE_SIZE: i64 = 8;

#[derive(Deserialize)]
pub struct AdminLoansRequest {
    #[serde(default = "default_page")]
    page: i64,
    /// `open` (pending + active — the ones that can still be defaulted),
    /// `defaulted`, or anything else for all of them.
    #[serde(default)]
    filter: String,
}

#[derive(Serialize)]
pub struct AdminInstallment {
    pub installment: i16,
    pub due_at: i64,
    pub principal_due: i64,
    pub interest_due: i64,
    pub principal_paid: i64,
    pub interest_paid: i64,
    pub status: String,
}

#[derive(Serialize)]
pub struct AdminCollateral {
    pub wallet_address: String,
    pub required_stroops: i64,
    pub locked_stroops: i64,
    pub status: String,
}

#[derive(Serialize)]
pub struct AdminRecovery {
    pub step: i16,
    pub source: String,
    pub username: Option<String>,
    pub amount: i64,
    pub stroops: Option<i64>,
    pub created_at: i64,
}

/// One thing that happened to a loan, in the order it happened.
///
/// The loan's whole life in one list — the collateral going into the vault and
/// coming back out, the pesos going to the borrower and coming back in. An
/// operator asking "what happened to this loan" should not have to read four
/// cards and a block explorer to answer it, and every on-chain row carries the
/// hash that proves it rather than asking anyone to take the row's word.
#[derive(Serialize)]
pub struct AdminMovement {
    /// `collateral_lock`, `collateral_mark_repaid`, `collateral_release`,
    /// `collateral_mark_defaulted`, `collateral_seize`, `disbursed`, `payout`
    /// or `payment`.
    pub kind: String,
    pub at: i64,
    /// `confirmed` / `queued` for the chain, the payout's own status for a
    /// transfer, `completed` for something that simply happened.
    pub status: String,
    /// Centavos, where the movement has a peso figure.
    pub amount: Option<i64>,
    pub stroops: Option<i64>,
    /// A Stellar transaction hash — the row's proof, and what the panel links
    /// to an explorer.
    pub tx_hash: Option<String>,
    /// PayPal's own reference, for the peso rows that have one.
    pub reference: Option<String>,
}

#[derive(Serialize)]
pub struct AdminLoan {
    pub id: Uuid,
    pub borrower: String,
    pub product: String,
    pub principal: i64,
    pub principal_outstanding: i64,
    pub rate_bps: i32,
    pub term_months: i16,
    pub status: String,
    pub disbursed_at: Option<i64>,
    pub defaulted_at: Option<i64>,
    pub closed_at: Option<i64>,
    pub schedule: Vec<AdminInstallment>,
    pub collateral: Option<AdminCollateral>,
    pub recoveries: Vec<AdminRecovery>,
    pub movements: Vec<AdminMovement>,
}

#[derive(Serialize)]
pub struct AdminLoansResponse {
    pub items: Vec<AdminLoan>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}

type LoanRow = (Uuid, String, String, i64, i64, i32, i16, String, Option<i64>, Option<i64>, Option<i64>);
/// loan_id, installment, due_at, principal_due, interest_due, principal_paid, interest_paid, status
type ScheduleRow = (Uuid, i16, i64, i64, i64, i64, i64, String);
/// loan_id, step, source, username, amount, stroops, created_at
type RecoveryRow = (Uuid, i16, String, Option<String>, i64, Option<i64>, i64);
/// loan_id, wallet_address, required_stroops, locked_stroops, status, lock_tx_hash, locked_at
type CollateralRow = (Uuid, String, i64, i64, String, Option<String>, Option<i64>);
/// loan_id, action, status, tx_hash, at, moved_stroops, value_centavos
type VaultRow = (Uuid, String, String, Option<String>, i64, Option<i64>, Option<i64>);
/// loan_id, amount, status, transaction_id, batch_id, created_at
type PayoutRow = (Uuid, i64, String, Option<String>, Option<String>, i64);
/// loan_id, amount_received, paid_at
type PaymentRow = (Uuid, i64, i64);

/// Assembles one loan's life from the rows already fetched for the page.
///
/// Chronological rather than newest-first: a loan is a story with an order —
/// locked, disbursed, paid out, repaid, released — and reading it forwards is
/// what makes a missing step obvious.
fn movements_for(
    loan_id: Uuid,
    disbursed_at: Option<i64>,
    principal: i64,
    collateral: &[CollateralRow],
    vault: &[VaultRow],
    payouts: &[PayoutRow],
    payments: &[PaymentRow],
) -> Vec<AdminMovement> {
    let mut out: Vec<AdminMovement> = Vec::new();

    // The borrower's own transaction, and the only movement they ever sign.
    if let Some((_, _, _, locked_stroops, _, Some(tx_hash), Some(locked_at))) =
        collateral.iter().find(|r| r.0 == loan_id)
    {
        out.push(AdminMovement {
            kind: "collateral_lock".to_string(),
            at: *locked_at,
            status: "confirmed".to_string(),
            amount: None,
            stroops: Some(*locked_stroops),
            tx_hash: Some(tx_hash.clone()),
            reference: None,
        });
    }

    if let Some(at) = disbursed_at {
        out.push(AdminMovement {
            kind: "disbursed".to_string(),
            at,
            status: "completed".to_string(),
            amount: Some(principal),
            stroops: None,
            tx_hash: None,
            reference: None,
        });
    }

    for (_, amount, status, transaction_id, batch_id, created_at) in payouts.iter().filter(|r| r.0 == loan_id) {
        out.push(AdminMovement {
            kind: "payout".to_string(),
            at: *created_at,
            status: status.clone(),
            amount: Some(*amount),
            stroops: None,
            tx_hash: None,
            reference: transaction_id.clone().or_else(|| batch_id.clone()),
        });
    }

    for (_, amount_received, paid_at) in payments.iter().filter(|r| r.0 == loan_id) {
        out.push(AdminMovement {
            kind: "payment".to_string(),
            at: *paid_at,
            status: "completed".to_string(),
            amount: Some(*amount_received),
            stroops: None,
            tx_hash: None,
            reference: None,
        });
    }

    for (_, action, status, tx_hash, at, moved_stroops, value_centavos) in vault.iter().filter(|r| r.0 == loan_id) {
        out.push(AdminMovement {
            kind: format!("collateral_{action}"),
            at: *at,
            // 'done' is the outbox's word; 'confirmed' is what it means — the
            // chain has been asked and answered.
            status: if status == "done" { "confirmed".to_string() } else { status.clone() },
            amount: *value_centavos,
            stroops: *moved_stroops,
            tx_hash: tx_hash.clone(),
            reference: None,
        });
    }

    out.sort_by_key(|m| m.at);
    out
}

pub async fn list(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(q): Json<AdminLoansRequest>,
) -> Result<Json<AdminLoansResponse>, E> {
    require_admin(&pool, &headers).await?;

    let page = q.page.max(1);
    let offset = (page - 1) * PAGE_SIZE;
    // Whitelisted at the door, never interpolated from the request — the
    // filter decides a SQL fragment, so it is chosen here and not received.
    let where_clause = match q.filter.as_str() {
        "open" => "WHERE l.status IN ('pending', 'active')",
        "defaulted" => "WHERE l.status = 'defaulted'",
        _ => "",
    };

    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM public.loans l {where_clause}"
    ))
    .fetch_one(&pool)
    .await
    .map_err(|e| db_err(e, "admin loans count"))?;

    let rows: Vec<LoanRow> = sqlx::query_as(&format!(
        "SELECT l.id, u.username, l.product, l.principal, l.principal_outstanding,
                l.rate_bps, l.term_months, l.status,
                l.disbursed_at, l.defaulted_at, l.closed_at
           FROM public.loans l
           JOIN public.users u ON u.id = l.borrower_id
          {where_clause}
          ORDER BY l.created_at DESC, l.id
          LIMIT $1 OFFSET $2"
    ))
    .bind(PAGE_SIZE)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "admin loans page"))?;

    let ids: Vec<Uuid> = rows.iter().map(|r| r.0).collect();

    let schedule: Vec<ScheduleRow> = sqlx::query_as(
        "SELECT loan_id, installment, due_at, principal_due, interest_due,
                principal_paid, interest_paid, status
           FROM public.loan_schedule
          WHERE loan_id = ANY($1)
          ORDER BY loan_id, installment",
    )
    .bind(&ids)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "admin schedule"))?;

    let collateral: Vec<CollateralRow> = sqlx::query_as(
        "SELECT loan_id, wallet_address, required_stroops, locked_stroops, status,
                lock_tx_hash, locked_at
           FROM public.xlm_collateral WHERE loan_id = ANY($1)",
    )
    .bind(&ids)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "admin collateral"))?;

    // Every vault movement, queued or settled — the queued ones matter most
    // here, since they are what the outbox above is waiting to sign.
    let vault: Vec<VaultRow> = sqlx::query_as(
        "SELECT c.loan_id, a.action, a.status, a.tx_hash,
                COALESCE(a.done_at, a.created_at), a.moved_stroops, a.value_centavos
           FROM public.collateral_actions a
           JOIN public.xlm_collateral c ON c.id = a.collateral_id
          WHERE c.loan_id = ANY($1)
          ORDER BY a.id",
    )
    .bind(&ids)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "admin vault movements"))?;

    // Money out (the disbursed proceeds) and money in (each repayment).
    let payouts: Vec<PayoutRow> = sqlx::query_as(
        "SELECT loan_id, amount, status, transaction_id, batch_id, created_at
           FROM public.payouts WHERE loan_id = ANY($1)",
    )
    .bind(&ids)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "admin payouts"))?;

    let payments: Vec<PaymentRow> = sqlx::query_as(
        "SELECT loan_id, amount_received, paid_at
           FROM public.loan_payments WHERE loan_id = ANY($1)
          ORDER BY paid_at",
    )
    .bind(&ids)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "admin payments"))?;

    let recoveries: Vec<RecoveryRow> = sqlx::query_as(
        "SELECT r.loan_id, r.step, r.source, u.username, r.amount, r.stroops, r.created_at
           FROM public.loan_recoveries r
           LEFT JOIN public.users u ON u.id = r.user_id
          WHERE r.loan_id = ANY($1)
          ORDER BY r.loan_id, r.step, r.id",
    )
    .bind(&ids)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "admin recoveries"))?;

    let items = rows
        .into_iter()
        .map(|(id, borrower, product, principal, principal_outstanding, rate_bps, term_months, status, disbursed_at, defaulted_at, closed_at)| {
            AdminLoan {
                schedule: schedule
                    .iter()
                    .filter(|r| r.0 == id)
                    .map(|(_, installment, due_at, principal_due, interest_due, principal_paid, interest_paid, status)| AdminInstallment {
                        installment: *installment,
                        due_at: *due_at,
                        principal_due: *principal_due,
                        interest_due: *interest_due,
                        principal_paid: *principal_paid,
                        interest_paid: *interest_paid,
                        status: status.clone(),
                    })
                    .collect(),
                collateral: collateral.iter().find(|r| r.0 == id).map(|(_, wallet_address, required_stroops, locked_stroops, status, _, _)| AdminCollateral {
                    wallet_address: wallet_address.clone(),
                    required_stroops: *required_stroops,
                    locked_stroops: *locked_stroops,
                    status: status.clone(),
                }),
                movements: movements_for(id, disbursed_at, principal, &collateral, &vault, &payouts, &payments),
                recoveries: recoveries
                    .iter()
                    .filter(|r| r.0 == id)
                    .map(|(_, step, source, username, amount, stroops, created_at)| AdminRecovery {
                        step: *step,
                        source: source.clone(),
                        username: username.clone(),
                        amount: *amount,
                        stroops: *stroops,
                        created_at: *created_at,
                    })
                    .collect(),
                id, borrower, product, principal, principal_outstanding, rate_bps,
                term_months, status, disbursed_at, defaulted_at, closed_at,
            }
        })
        .collect();

    let total_pages = if total == 0 { 1 } else { (total + PAGE_SIZE - 1) / PAGE_SIZE };

    Ok(Json(AdminLoansResponse { items, total, page, page_size: PAGE_SIZE, total_pages }))
}
