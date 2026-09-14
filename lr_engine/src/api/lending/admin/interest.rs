//! POST /lending/admin/interest — where every repayment's interest went.
//!
//! One item per repayment, newest first: the payment's five-way split, and
//! every member who received a slice of it with the numbers that explain the
//! slice — their deposit balance against the pool's, or their pledge and tier
//! against the loan's pledges. The platform, reserve and recovery-fund parts
//! are on the item itself, so an operator can account for every centavo of a
//! payment from one row.
//!
//! Member slices that rounded to nothing were never recorded (they moved no
//! money), so `recipients` lists the members who were actually paid.
//!
//! Batched like `loans`: the page's event ids are collected once and every
//! recipient row is fetched with `= ANY($1)`, so a page costs three queries.

use axum::{Extension, Json, http::HeaderMap};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::lending::domain::InterestParts;
use crate::api::lending::shared::db_err;
use crate::api::users::shared::{E, require_admin};

fn default_page() -> i64 {
    1
}
/// Fixed server-side, same rationale as `loans::PAGE_SIZE`.
const PAGE_SIZE: i64 = 8;
/// A search term longer than any username is not a search.
const MAX_SEARCH: usize = 64;

#[derive(Deserialize)]
pub struct AdminInterestRequest {
    #[serde(default = "default_page")]
    page: i64,
    /// Narrows to repayments where this text appears in the borrower's
    /// username or in any paid member's. Case-insensitive.
    #[serde(default)]
    search: String,
}

#[derive(Serialize)]
pub struct InterestRecipient {
    pub username: String,
    /// `depositor` or `guarantor`.
    pub role: String,
    /// What the slice was proportional to: the member's deposit balance for a
    /// depositor, their pledge for a guarantor.
    pub weight: i64,
    /// The tier percent a guarantor was paid at.
    pub tier_share: Option<i16>,
    pub amount: i64,
}

#[derive(Serialize)]
pub struct InterestPayment {
    pub event_id: i64,
    pub loan_id: Uuid,
    pub product: String,
    pub borrower: String,
    pub paid_at: i64,
    pub interest: i64,
    pub parts: InterestParts,
    /// The whole pool's deposit balance the depositors' share was divided by.
    /// None on repayments recorded before it was captured (040).
    pub pool_balance: Option<i64>,
    /// The loan's accepted pledges the guarantors' share was weighted by.
    pub pledged_total: Option<i64>,
    pub policy_version: Option<i64>,
    pub recipients: Vec<InterestRecipient>,
}

#[derive(Serialize)]
pub struct AdminInterestResponse {
    pub items: Vec<InterestPayment>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}

/// event_id, loan_id, product, borrower, paid_at, interest, platform, reserve,
/// depositors, guarantor, recovery_fund, pool_balance, pledged_total, policy_version
type PaymentRow = (i64, Uuid, String, String, i64, i64, i64, i64, i64, i64, i64, Option<i64>, Option<i64>, Option<i64>);
/// event_id, username, role, weight, tier_share, amount
type RecipientRow = (i64, String, String, i64, Option<i16>, i64);

/// The page's filter, shared by the count and the page so they can't disagree.
/// `$1` is the search text, or '' for everything.
const MATCHING: &str = "
      FROM public.interest_splits s
      JOIN public.loans l ON l.id = s.loan_id
      JOIN public.users b ON b.id = l.borrower_id
     WHERE $1 = ''
        OR position(lower($1) in lower(b.username)) > 0
        OR EXISTS (
             SELECT 1 FROM public.member_interest m
               JOIN public.users mu ON mu.id = m.user_id
              WHERE m.event_id = s.event_id
                AND position(lower($1) in lower(mu.username)) > 0
           )
";

pub async fn list(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(q): Json<AdminInterestRequest>,
) -> Result<Json<AdminInterestResponse>, E> {
    require_admin(&pool, &headers).await?;

    let page = q.page.max(1);
    let offset = (page - 1) * PAGE_SIZE;
    // Bound, not validated: `position` treats it as plain text, so there is
    // nothing in it to escape.
    let search: String = q.search.trim().chars().take(MAX_SEARCH).collect();

    let total: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) {MATCHING}"))
        .bind(&search)
        .fetch_one(&pool)
        .await
        .map_err(|e| db_err(e, "admin interest count"))?;

    let rows: Vec<PaymentRow> = sqlx::query_as(&format!(
        "SELECT s.event_id, s.loan_id, l.product, b.username, s.created_at, s.interest,
                s.platform, s.reserve, s.depositors, s.guarantor, s.recovery_fund,
                s.pool_balance, s.pledged_total, s.policy_version
         {MATCHING}
          ORDER BY s.created_at DESC, s.id DESC
          LIMIT $2 OFFSET $3"
    ))
    .bind(&search)
    .bind(PAGE_SIZE)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "admin interest page"))?;

    let event_ids: Vec<i64> = rows.iter().map(|r| r.0).collect();
    // Guarantor slices first (they are per loan and few), then depositors by
    // size, so the list reads from "why this loan" to "the rest of the pool".
    let recipients: Vec<RecipientRow> = sqlx::query_as(
        "SELECT m.event_id, u.username, m.role, m.weight, m.tier_share, m.amount
           FROM public.member_interest m
           JOIN public.users u ON u.id = m.user_id
          WHERE m.event_id = ANY($1)
          ORDER BY m.event_id, m.role DESC, m.amount DESC, u.username",
    )
    .bind(&event_ids)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "admin interest recipients"))?;

    let items = rows
        .into_iter()
        .map(
            |(event_id, loan_id, product, borrower, paid_at, interest, platform, reserve, depositors, guarantor, recovery_fund, pool_balance, pledged_total, policy_version)| {
                InterestPayment {
                    event_id,
                    loan_id,
                    product,
                    borrower,
                    paid_at,
                    interest,
                    parts: InterestParts { platform, reserve, depositors, guarantor, recovery_fund },
                    pool_balance,
                    pledged_total,
                    policy_version,
                    recipients: recipients
                        .iter()
                        .filter(|r| r.0 == event_id)
                        .map(|(_, username, role, weight, tier_share, amount)| InterestRecipient {
                            username: username.clone(),
                            role: role.clone(),
                            weight: *weight,
                            tier_share: *tier_share,
                            amount: *amount,
                        })
                        .collect(),
                }
            },
        )
        .collect();

    Ok(Json(AdminInterestResponse {
        items,
        total,
        page,
        page_size: PAGE_SIZE,
        total_pages: if total == 0 { 1 } else { (total + PAGE_SIZE - 1) / PAGE_SIZE },
    }))
}
