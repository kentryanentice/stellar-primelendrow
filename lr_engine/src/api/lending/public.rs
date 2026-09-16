//! GET /public/loans and GET /public/loans/{id} — the loan book, open to
//! anyone, with nobody in it.
//!
//! Every loan the platform has ever taken an application for — pending,
//! declined, cancelled, active, repaid, defaulted, settled — with what backed
//! it, every repayment and how its interest was split, and what recovery took
//! when it went bad. A member, a depositor or a reviewer can check the books
//! without an account and without taking anyone's word for them.
//!
//! What it must never carry is a person. No user id, username, email, wallet
//! address, PayPal reference or score leaves this module: a loan is known by
//! its reference, a guarantor by their position on that loan ("Guarantor 2"),
//! and the depositors a payment paid by how many there were and what they got
//! in total. That is enforced by construction rather than by care — every
//! response is a struct declared here, built field by field from queries that
//! only select what the struct carries, and `views_carry_no_personal_fields`
//! fails the build if a personal field name ever appears in one. User ids are
//! read in exactly one place (to number the guarantors), and never serialized.
//!
//! The Stellar transaction hashes are published on purpose: the chain is
//! already public, and a hash is the proof that the collateral moved.

use std::collections::HashMap;

use axum::{
    Extension, Json,
    extract::{Path, Query},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::domain::InterestParts;
use super::shared::db_err;
use crate::api::users::shared::E;

fn default_page() -> i64 {
    1
}
/// Fixed server-side, same rationale as deposits_list::PAGE_SIZE.
const PAGE_SIZE: i64 = 12;

#[derive(Deserialize)]
pub struct PublicLoansQuery {
    #[serde(default = "default_page")]
    page: i64,
    /// One of the loan statuses, or a group — see `status_clause`.
    #[serde(default)]
    status: String,
    /// `deposit_backed`, `xlm_collateral`, `guarantor`, or anything else for all.
    #[serde(default)]
    product: String,
}

/// What every loan row shows, in the list and at the top of its own page.
/// Read straight off `LOAN_SELECT`, whose aliases are these field names.
#[derive(Serialize, sqlx::FromRow)]
pub struct PublicLoan {
    /// The loan reference. Random, and linked to nobody outside the engine.
    pub id: Uuid,
    pub product: String,
    pub status: String,
    pub principal: i64,
    pub principal_outstanding: i64,
    pub rate_bps: i32,
    pub term_months: i16,
    pub applied_at: i64,
    /// Last change to the row — when a declined or cancelled application was
    /// decided, since those have no date column of their own.
    pub updated_at: i64,
    pub disbursed_at: Option<i64>,
    pub closed_at: Option<i64>,
    pub defaulted_at: Option<i64>,
    pub reconciled_at: Option<i64>,
    /// The borrower's own deposit locked behind the loan: what was locked when
    /// it was disbursed, or what is locked right now if it has not been.
    pub deposit_locked: i64,
    /// XLM the borrower locked on chain, in stroops. None when the loan has no
    /// XLM leg.
    pub xlm_required_stroops: Option<i64>,
    pub xlm_locked_stroops: Option<i64>,
    /// Deposits guarantors actually locked (accepted, and later released or
    /// seized). Invitations that were declined or never answered are not money.
    pub guarantor_locked: i64,
    pub guarantors: i64,
    pub payments: i64,
    pub repaid: i64,
    pub interest_paid: i64,
}

#[derive(Serialize)]
pub struct StatusCount {
    pub status: String,
    pub count: i64,
}

/// The whole book in four numbers, over every loan whatever the filter.
#[derive(Serialize)]
pub struct PublicSummary {
    pub loans: i64,
    pub by_status: Vec<StatusCount>,
    pub disbursed: i64,
    pub repaid: i64,
    pub interest: i64,
}

#[derive(Serialize)]
pub struct PublicLoansResponse {
    pub items: Vec<PublicLoan>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
    pub summary: PublicSummary,
}

#[derive(Serialize)]
pub struct PublicInstallment {
    pub installment: i16,
    pub due_at: i64,
    pub principal_due: i64,
    pub interest_due: i64,
    pub principal_paid: i64,
    pub interest_paid: i64,
    pub status: String,
}

#[derive(Serialize)]
pub struct PublicVaultAction {
    /// `release`, `seize`, or the vault's mark-* bookkeeping actions.
    pub action: String,
    /// `queued` until the chain has answered, then `done`.
    pub status: String,
    pub tx_hash: Option<String>,
    pub at: i64,
    pub moved_stroops: Option<i64>,
    pub value_centavos: Option<i64>,
}

#[derive(Serialize)]
pub struct PublicCollateral {
    pub required_stroops: i64,
    pub locked_stroops: i64,
    /// `pending`, `locked`, `released` or `seized`.
    pub status: String,
    pub lock_tx_hash: Option<String>,
    pub locked_at: Option<i64>,
    pub collateral_ratio_bps: i32,
    /// The pesos this XLM stands behind.
    pub covers_centavos: Option<i64>,
    pub actions: Vec<PublicVaultAction>,
}

#[derive(Serialize)]
pub struct PublicGuarantor {
    /// 1-based, in the order they were asked. The only name a guarantor has here.
    pub position: i64,
    pub pledge_amount: i64,
    /// `invited`, `accepted`, `declined`, `released` or `seized`.
    pub status: String,
}

/// What is still locked behind the loan at this moment, by badge.
#[derive(Serialize)]
pub struct PublicLockedNow {
    /// The borrower's own deposit.
    pub collateral: i64,
    /// Depositors' money funding the loan, pro-rata.
    pub lent: i64,
    /// Guarantors' deposits.
    pub pledged: i64,
}

#[derive(Serialize)]
pub struct PublicGuarantorSlice {
    pub position: Option<i64>,
    pub tier_share: Option<i16>,
    pub amount: i64,
}

#[derive(Serialize)]
pub struct PublicSplit {
    pub interest: i64,
    pub parts: InterestParts,
    pub policy_version: Option<i64>,
    /// Every member balance the depositors' share was divided over.
    pub pool_balance: Option<i64>,
    pub pledged_total: Option<i64>,
    /// How many members the depositors' share reached — counted, not named.
    pub depositors_paid: i64,
    pub guarantors: Vec<PublicGuarantorSlice>,
}

#[derive(Serialize)]
pub struct PublicPayment {
    pub paid_at: i64,
    /// Applied to the loan: exactly the installment that was due.
    pub amount: i64,
    pub principal_paid: i64,
    pub interest_paid: i64,
    /// The payment provider's fee, paid by the borrower on top.
    pub fee_paid: i64,
    /// None for a payment with no interest in it, or one booked before
    /// splits were recorded (037).
    pub split: Option<PublicSplit>,
}

#[derive(Serialize)]
pub struct PublicRecovery {
    pub step: i16,
    /// `borrower_deposit`, `borrower_xlm`, `guarantor_deposit`,
    /// `recovery_fund` or `reserve_fund`.
    pub source: String,
    pub guarantor: Option<i64>,
    pub amount: i64,
    /// Given back by a settlement after the default (033).
    pub refunded: i64,
    pub stroops: Option<i64>,
    pub at: i64,
}

#[derive(Serialize)]
pub struct PublicLoanDetail {
    pub loan: PublicLoan,
    pub policy_version: i64,
    /// What the borrower had to cover themselves before guarantors (031).
    pub borrower_cover: i64,
    /// Taken pro-rata from every member's balance at disbursement.
    pub pool_funded: Option<i64>,
    pub locked_now: PublicLockedNow,
    pub collateral: Option<PublicCollateral>,
    pub guarantors: Vec<PublicGuarantor>,
    pub schedule: Vec<PublicInstallment>,
    pub payments: Vec<PublicPayment>,
    pub recoveries: Vec<PublicRecovery>,
}

/// The loan columns plus the per-loan aggregates, aliased to `PublicLoan`'s
/// fields. Correlated subqueries rather than joins so no aggregate can
/// multiply another's rows.
const LOAN_SELECT: &str = "
    SELECT l.id, l.product, l.status, l.principal, l.principal_outstanding,
           l.rate_bps, l.term_months, l.created_at AS applied_at, l.updated_at,
           l.disbursed_at, l.closed_at, l.defaulted_at, l.reconciled_at,
           COALESCE(
               (SELECT (e.payload->>'own_deposit_backing')::BIGINT
                  FROM public.ledger_events e
                 WHERE e.loan_id = l.id AND e.kind = 'loan_disbursed'
                 ORDER BY e.id LIMIT 1),
               (SELECT SUM(d.amount) FROM public.deposits d
                 WHERE d.backing_loan = l.id AND d.badge = 'collateral'),
               0)::BIGINT AS deposit_locked,
           c.required_stroops AS xlm_required_stroops,
           c.locked_stroops AS xlm_locked_stroops,
           (SELECT COALESCE(SUM(g.pledge_amount), 0) FROM public.loan_guarantors g
             WHERE g.loan_id = l.id AND g.status IN ('accepted', 'released', 'seized'))::BIGINT
               AS guarantor_locked,
           (SELECT COUNT(*) FROM public.loan_guarantors g WHERE g.loan_id = l.id)::BIGINT AS guarantors,
           (SELECT COUNT(*) FROM public.loan_payments p WHERE p.loan_id = l.id)::BIGINT AS payments,
           (SELECT COALESCE(SUM(p.amount_received), 0) FROM public.loan_payments p
             WHERE p.loan_id = l.id)::BIGINT AS repaid,
           (SELECT COALESCE(SUM(p.interest_paid), 0) FROM public.loan_payments p
             WHERE p.loan_id = l.id)::BIGINT AS interest_paid
      FROM public.loans l
      LEFT JOIN public.xlm_collateral c ON c.loan_id = l.id
";

/// Whitelisted at the door, never interpolated from the request.
fn status_clause(status: &str) -> &'static str {
    match status {
        "pending" => "l.status = 'pending'",
        "active" => "l.status = 'active'",
        "closed" => "l.status = 'closed'",
        "declined" => "l.status = 'declined'",
        "cancelled" => "l.status = 'cancelled'",
        // A default and the way back from one are one story.
        "defaulted" => "l.status IN ('defaulted', 'reconciling', 'reconciled')",
        _ => "TRUE",
    }
}

fn product_clause(product: &str) -> &'static str {
    match product {
        "deposit_backed" => "l.product = 'deposit_backed'",
        "xlm_collateral" => "l.product = 'xlm_collateral'",
        "guarantor" => "l.product = 'guarantor'",
        _ => "TRUE",
    }
}

/// Numbers a loan's guarantors 1, 2, 3 in the order they were asked, keyed by
/// the user id the rows below arrive with. The ids stay in this map; only the
/// positions are ever serialized.
fn guarantor_positions(ordered_ids: &[Uuid]) -> HashMap<Uuid, i64> {
    ordered_ids.iter().enumerate().map(|(i, id)| (*id, i as i64 + 1)).collect()
}

pub async fn list(
    Extension(pool): Extension<PgPool>,
    Query(q): Query<PublicLoansQuery>,
) -> Result<Json<PublicLoansResponse>, E> {
    let page = q.page.clamp(1, 1_000_000);
    let offset = (page - 1) * PAGE_SIZE;
    let where_clause = format!("WHERE {} AND {}", status_clause(&q.status), product_clause(&q.product));

    let total: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM public.loans l {where_clause}"))
        .fetch_one(&pool)
        .await
        .map_err(|e| db_err(e, "public loans count"))?;

    let items: Vec<PublicLoan> = sqlx::query_as(&format!(
        "{LOAN_SELECT} {where_clause} ORDER BY l.created_at DESC, l.id LIMIT $1 OFFSET $2"
    ))
    .bind(PAGE_SIZE)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "public loans page"))?;

    let by_status: Vec<(String, i64)> = sqlx::query_as(
        "SELECT status, COUNT(*) FROM public.loans GROUP BY status ORDER BY status",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "public loans by status"))?;

    let (disbursed, repaid, interest): (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COALESCE(SUM(principal), 0) FROM public.loans WHERE disbursed_at IS NOT NULL)::BIGINT,
                (SELECT COALESCE(SUM(amount_received), 0) FROM public.loan_payments)::BIGINT,
                (SELECT COALESCE(SUM(interest_paid), 0) FROM public.loan_payments)::BIGINT",
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| db_err(e, "public loans totals"))?;

    let summary = PublicSummary {
        loans: by_status.iter().map(|(_, n)| n).sum(),
        by_status: by_status.into_iter().map(|(status, count)| StatusCount { status, count }).collect(),
        disbursed,
        repaid,
        interest,
    };

    Ok(Json(PublicLoansResponse {
        items,
        total,
        page,
        page_size: PAGE_SIZE,
        total_pages: if total == 0 { 1 } else { (total + PAGE_SIZE - 1) / PAGE_SIZE },
        summary,
    }))
}

/// installment, due_at, principal_due, interest_due, principal_paid, interest_paid, status
type ScheduleRow = (i16, i64, i64, i64, i64, i64, String);
/// required_stroops, locked_stroops, status, lock_tx_hash, locked_at, collateral_ratio_bps, principal_centavos
type CollateralRow = (i64, i64, String, Option<String>, Option<i64>, i32, Option<i64>);
/// action, status, tx_hash, at, moved_stroops, value_centavos
type VaultRow = (String, String, Option<String>, i64, Option<i64>, Option<i64>);
/// paid_at, amount_received, principal_paid, interest_paid, fee_paid, event_id,
/// interest, platform, reserve, depositors, guarantor, recovery_fund,
/// policy_version, pool_balance, pledged_total
type PaymentRow = (
    i64, i64, i64, i64, i64, Option<i64>,
    Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<i64>,
    Option<i64>, Option<i64>, Option<i64>,
);
/// event_id, user_id, role, tier_share, amount — user_id only to find the position
type SliceRow = (i64, Uuid, String, Option<i16>, i64);
/// step, source, user_id, amount, refunded, stroops, created_at
type RecoveryRow = (i16, String, Option<Uuid>, i64, i64, Option<i64>, i64);

pub async fn detail(
    Extension(pool): Extension<PgPool>,
    Path(loan_id): Path<Uuid>,
) -> Result<Json<PublicLoanDetail>, E> {
    let loan: PublicLoan = sqlx::query_as(&format!("{LOAN_SELECT} WHERE l.id = $1"))
        .bind(loan_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| db_err(e, "public loan"))?
        .ok_or((StatusCode::NOT_FOUND, "No such loan"))?;

    let (policy_version, borrower_cover, pool_funded): (i64, i64, Option<i64>) = sqlx::query_as(
        "SELECT l.policy_version, l.borrower_cover_centavos,
                (SELECT (e.payload->>'pool_funded')::BIGINT FROM public.ledger_events e
                  WHERE e.loan_id = l.id AND e.kind = 'loan_disbursed'
                  ORDER BY e.id LIMIT 1)
           FROM public.loans l WHERE l.id = $1",
    )
    .bind(loan_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| db_err(e, "public loan terms"))?;

    let (collateral_now, lent_now, pledged_now): (i64, i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(amount) FILTER (WHERE badge = 'collateral'), 0)::BIGINT,
                COALESCE(SUM(amount) FILTER (WHERE badge = 'lent'), 0)::BIGINT,
                COALESCE(SUM(amount) FILTER (WHERE badge = 'pledged'), 0)::BIGINT
           FROM public.deposits WHERE backing_loan = $1",
    )
    .bind(loan_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| db_err(e, "public loan locks"))?;

    let guarantor_rows: Vec<(Uuid, i64, String)> = sqlx::query_as(
        "SELECT guarantor_id, pledge_amount, status FROM public.loan_guarantors
          WHERE loan_id = $1 ORDER BY created_at, id",
    )
    .bind(loan_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "public loan guarantors"))?;
    let positions = guarantor_positions(&guarantor_rows.iter().map(|r| r.0).collect::<Vec<_>>());
    let guarantors = guarantor_rows
        .into_iter()
        .map(|(id, pledge_amount, status)| PublicGuarantor { position: positions[&id], pledge_amount, status })
        .collect();

    let schedule: Vec<ScheduleRow> = sqlx::query_as(
        "SELECT installment, due_at, principal_due, interest_due, principal_paid, interest_paid, status
           FROM public.loan_schedule WHERE loan_id = $1 ORDER BY installment",
    )
    .bind(loan_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "public loan schedule"))?;

    let collateral_row: Option<CollateralRow> = sqlx::query_as(
        "SELECT required_stroops, locked_stroops, status, lock_tx_hash, locked_at,
                collateral_ratio_bps, principal_centavos
           FROM public.xlm_collateral WHERE loan_id = $1",
    )
    .bind(loan_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| db_err(e, "public loan collateral"))?;

    let collateral = match collateral_row {
        None => None,
        Some((required_stroops, locked_stroops, status, lock_tx_hash, locked_at, collateral_ratio_bps, covers_centavos)) => {
            let vault: Vec<VaultRow> = sqlx::query_as(
                "SELECT a.action, a.status, a.tx_hash, COALESCE(a.done_at, a.created_at),
                        a.moved_stroops, a.value_centavos
                   FROM public.collateral_actions a
                   JOIN public.xlm_collateral c ON c.id = a.collateral_id
                  WHERE c.loan_id = $1
                  ORDER BY a.id",
            )
            .bind(loan_id)
            .fetch_all(&pool)
            .await
            .map_err(|e| db_err(e, "public loan vault"))?;
            Some(PublicCollateral {
                required_stroops,
                locked_stroops,
                status,
                lock_tx_hash,
                locked_at,
                collateral_ratio_bps,
                covers_centavos,
                actions: vault
                    .into_iter()
                    .map(|(action, status, tx_hash, at, moved_stroops, value_centavos)| PublicVaultAction {
                        action, status, tx_hash, at, moved_stroops, value_centavos,
                    })
                    .collect(),
            })
        }
    };

    // A payment and its split share the capture reference: loan_payments keeps
    // it as rail_ref, the ledger event it booked carries the same one, and the
    // split hangs off that event. The reference itself is never selected out.
    let payment_rows: Vec<PaymentRow> = sqlx::query_as(
        "SELECT p.paid_at, p.amount_received, p.principal_paid, p.interest_paid, p.fee_paid,
                s.event_id, s.interest, s.platform, s.reserve, s.depositors, s.guarantor,
                s.recovery_fund, s.policy_version, s.pool_balance, s.pledged_total
           FROM public.loan_payments p
           LEFT JOIN public.ledger_events e ON e.rail_ref = p.rail_ref
           LEFT JOIN public.interest_splits s ON s.event_id = e.id
          WHERE p.loan_id = $1
          ORDER BY p.paid_at, p.id",
    )
    .bind(loan_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "public loan payments"))?;

    // By event rather than by loan: (event_id, user_id, role) is the index.
    let event_ids: Vec<i64> = payment_rows.iter().filter_map(|r| r.5).collect();
    let slices: Vec<SliceRow> = sqlx::query_as(
        "SELECT event_id, user_id, role, tier_share, amount
           FROM public.member_interest WHERE event_id = ANY($1)
          ORDER BY event_id, amount DESC, id",
    )
    .bind(&event_ids)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "public loan interest slices"))?;

    let payments = payment_rows
        .into_iter()
        .map(|(paid_at, amount, principal_paid, interest_paid, fee_paid, event_id, interest, platform, reserve, depositors, guarantor, recovery_fund, policy_version, pool_balance, pledged_total)| {
            let split = match (event_id, interest, platform, reserve, depositors, guarantor, recovery_fund) {
                (Some(event_id), Some(interest), Some(platform), Some(reserve), Some(depositors), Some(guarantor), Some(recovery_fund)) => {
                    let of_event = || slices.iter().filter(move |s| s.0 == event_id);
                    Some(PublicSplit {
                        interest,
                        parts: InterestParts { platform, reserve, depositors, guarantor, recovery_fund },
                        policy_version,
                        pool_balance,
                        pledged_total,
                        depositors_paid: of_event().filter(|s| s.2 == "depositor").count() as i64,
                        guarantors: of_event()
                            .filter(|s| s.2 == "guarantor")
                            .map(|(_, user_id, _, tier_share, amount)| PublicGuarantorSlice {
                                position: positions.get(user_id).copied(),
                                tier_share: *tier_share,
                                amount: *amount,
                            })
                            .collect(),
                    })
                }
                _ => None,
            };
            PublicPayment { paid_at, amount, principal_paid, interest_paid, fee_paid, split }
        })
        .collect();

    let recoveries: Vec<RecoveryRow> = sqlx::query_as(
        "SELECT step, source, user_id, amount, refunded, stroops, created_at
           FROM public.loan_recoveries WHERE loan_id = $1
          ORDER BY step, id",
    )
    .bind(loan_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "public loan recoveries"))?;

    Ok(Json(PublicLoanDetail {
        loan,
        policy_version,
        borrower_cover,
        pool_funded,
        locked_now: PublicLockedNow { collateral: collateral_now, lent: lent_now, pledged: pledged_now },
        collateral,
        guarantors,
        schedule: schedule
            .into_iter()
            .map(|(installment, due_at, principal_due, interest_due, principal_paid, interest_paid, status)| PublicInstallment {
                installment, due_at, principal_due, interest_due, principal_paid, interest_paid, status,
            })
            .collect(),
        payments,
        recoveries: recoveries
            .into_iter()
            .map(|(step, source, user_id, amount, refunded, stroops, at)| PublicRecovery {
                // Only a guarantor step names anyone, and it names them by position.
                guarantor: if source == "guarantor_deposit" {
                    user_id.and_then(|id| positions.get(&id).copied())
                } else {
                    None
                },
                step, source, amount, refunded, stroops, at,
            })
            .collect(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Field names that would put a person on a public page.
    const PERSONAL: &[&str] = &[
        "user_id", "borrower_id", "guarantor_id", "borrower", "username", "email", "wallet",
        "wallet_address", "payer_id", "rail_ref", "reference", "capture_id", "score", "actor_id",
        "name", "phone", "address",
    ];

    fn keys(value: &serde_json::Value, out: &mut Vec<String>) {
        match value {
            serde_json::Value::Object(map) => {
                for (k, v) in map {
                    out.push(k.clone());
                    keys(v, out);
                }
            }
            serde_json::Value::Array(items) => items.iter().for_each(|v| keys(v, out)),
            _ => {}
        }
    }

    fn sample_loan() -> PublicLoan {
        PublicLoan {
            id: Uuid::nil(), product: "guarantor".into(), status: "defaulted".into(), principal: 1,
            principal_outstanding: 0, rate_bps: 1, term_months: 3, applied_at: 0, updated_at: 0,
            disbursed_at: Some(0), closed_at: Some(0), defaulted_at: Some(0), reconciled_at: Some(0),
            deposit_locked: 0, xlm_required_stroops: Some(1), xlm_locked_stroops: Some(1),
            guarantor_locked: 0, guarantors: 1, payments: 1, repaid: 0, interest_paid: 0,
        }
    }

    /// Every field of every public view, filled so nothing is skipped, and
    /// not one of them may be a personal field.
    #[test]
    fn views_carry_no_personal_fields() {
        let detail = PublicLoanDetail {
            loan: sample_loan(),
            policy_version: 1,
            borrower_cover: 0,
            pool_funded: Some(0),
            locked_now: PublicLockedNow { collateral: 0, lent: 0, pledged: 0 },
            collateral: Some(PublicCollateral {
                required_stroops: 1, locked_stroops: 1, status: "locked".into(), lock_tx_hash: Some("h".into()),
                locked_at: Some(0), collateral_ratio_bps: 12000, covers_centavos: Some(1),
                actions: vec![PublicVaultAction {
                    action: "seize".into(), status: "done".into(), tx_hash: Some("h".into()), at: 0,
                    moved_stroops: Some(1), value_centavos: Some(1),
                }],
            }),
            guarantors: vec![PublicGuarantor { position: 1, pledge_amount: 1, status: "seized".into() }],
            schedule: vec![PublicInstallment {
                installment: 1, due_at: 0, principal_due: 1, interest_due: 1, principal_paid: 0,
                interest_paid: 0, status: "defaulted".into(),
            }],
            payments: vec![PublicPayment {
                paid_at: 0, amount: 1, principal_paid: 0, interest_paid: 1, fee_paid: 0,
                split: Some(PublicSplit {
                    interest: 1,
                    parts: InterestParts { platform: 0, reserve: 0, depositors: 1, guarantor: 0, recovery_fund: 0 },
                    policy_version: Some(1), pool_balance: Some(1), pledged_total: Some(1), depositors_paid: 1,
                    guarantors: vec![PublicGuarantorSlice { position: Some(1), tier_share: Some(10), amount: 0 }],
                }),
            }],
            recoveries: vec![PublicRecovery {
                step: 3, source: "guarantor_deposit".into(), guarantor: Some(1), amount: 1, refunded: 0,
                stroops: None, at: 0,
            }],
        };
        let list = PublicLoansResponse {
            items: vec![sample_loan()],
            total: 1, page: 1, page_size: PAGE_SIZE, total_pages: 1,
            summary: PublicSummary {
                loans: 1, by_status: vec![StatusCount { status: "active".into(), count: 1 }],
                disbursed: 0, repaid: 0, interest: 0,
            },
        };

        let mut found = Vec::new();
        keys(&serde_json::to_value(&detail).unwrap(), &mut found);
        keys(&serde_json::to_value(&list).unwrap(), &mut found);
        for key in &found {
            assert!(!PERSONAL.contains(&key.as_str()), "public view exposes `{key}`");
        }
    }

    #[test]
    fn guarantors_are_numbered_in_asking_order() {
        let (a, b, c) = (Uuid::from_u128(3), Uuid::from_u128(1), Uuid::from_u128(2));
        let positions = guarantor_positions(&[a, b, c]);
        assert_eq!((positions[&a], positions[&b], positions[&c]), (1, 2, 3));
    }

    #[test]
    fn filters_only_ever_pick_a_fixed_fragment() {
        assert_eq!(status_clause("'; DROP TABLE loans; --"), "TRUE");
        assert_eq!(product_clause("guarantor OR 1=1"), "TRUE");
        assert_eq!(status_clause("defaulted"), "l.status IN ('defaulted', 'reconciling', 'reconciled')");
    }
}
