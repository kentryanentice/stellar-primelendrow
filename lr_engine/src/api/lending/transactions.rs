//! POST /pool/transactions — the caller's money movements, paginated.
//!
//! "Where did my money go" answered once, for every rail the member touches:
//! pesos into the pool (a captured PayPal order), pesos back out (a PayPal
//! payout, 029), and XLM in and out of the vault contract. The cards elsewhere
//! answer narrower questions — `deposits_list` shows lots that still EXIST,
//! `custody` shows one loan's on-chain evidence — and neither is a history: a
//! lot consumed by a withdrawal is deleted, so nothing on the Lend page could
//! previously show that the withdrawal ever happened.
//!
//! Assembled from what already exists rather than a new mirror table, the same
//! call `custody` makes. Five sources, one ordered list:
//!
//!   * deposits and withdrawals from `ledger_events` + `ledger_postings` —
//!     the amount is read from the POSTING, never from the payload, so the
//!     number in the record is the number in the books;
//!   * the lock from `xlm_collateral`, which holds the borrower's own
//!     transaction hash and the moment it was verified on-chain;
//!   * releases and seizures from `collateral_actions`; and
//!   * deposits lost to a default from `loan_recoveries` (030) — which reach
//!     guarantors too, so this is the one source where a row on a member's
//!     record belongs to somebody else's loan.
//!
//! Only movements are listed. `mark_repaid` and `mark_defaulted` are on-chain
//! state changes that move no coins, so they stay in the per-loan custody
//! record rather than padding a transaction list with rows carrying an amount
//! that never went anywhere.
//!
//! Every row is scoped to the caller by `user_id`/`borrower_id` inside the
//! query itself — there is no post-filtering step to forget.

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
const PAGE_SIZE: i64 = 8;

#[derive(Deserialize)]
pub struct TransactionsRequest {
    #[serde(default = "default_page")]
    page: i64,
}

#[derive(Serialize)]
pub struct TransactionView {
    /// `<source>:<id>`. These rows come from four tables with four id
    /// sequences, so there is no single number that could key them.
    pub id: String,
    /// `deposit`, `withdrawal`, `collateral_lock`, `collateral_release` or
    /// `collateral_seize`.
    pub kind: String,
    /// The unit `amount` is in: `php` (centavos) or `xlm` (stroops). A
    /// transaction list that mixed the two without saying which is which would
    /// be worse than no list.
    pub asset: String,
    /// Always positive — which way it went is the `kind`'s job to say.
    pub amount: i64,
    /// Where the movement got to. Deposits are `completed` (the capture id is
    /// the proof); withdrawals carry their payout's own status, or `recorded`
    /// for one made before 029's rail existed; collateral is `confirmed` once
    /// a transaction hash is on record and `queued` while it waits for the
    /// admin key.
    pub status: String,
    /// The provider's own reference — a PayPal capture or transfer id, or a
    /// Stellar transaction hash — so the member can check the movement
    /// somewhere that isn't us.
    pub reference: Option<String>,
    pub loan_id: Option<Uuid>,
    pub at: i64,
}

#[derive(Serialize)]
pub struct TransactionsResponse {
    pub items: Vec<TransactionView>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}

/// The union of every movement belonging to `$1`, unordered and unpaginated.
/// Written once and used by both the count and the page below, so the two can
/// never disagree about what a transaction is.
const MOVEMENTS: &str = "
    -- pesos in: a PayPal order captured server-side became a deposit lot.
    -- member_deposits is credit-normal, so the posting is negative here.
    -- The casts on this branch are what fix the whole union's column types:
    -- every later branch is coerced to what the first one declares.
    SELECT e.created_at                            AS at,
           'deposit'::text                         AS kind,
           'php'::text                             AS asset,
           -p.amount                               AS amount,
           'completed'::text                       AS status,
           e.rail_ref::text                        AS reference,
           NULL::uuid                              AS loan_id,
           'deposit:' || e.id::text                AS id
      FROM public.ledger_events e
      JOIN public.ledger_postings p
        ON p.event_id = e.id AND p.account = 'member_deposits'
     WHERE e.user_id = $1 AND e.kind = 'deposit_confirmed'

    UNION ALL

    -- pesos out: withdrawable balance sent to the member's own PayPal. The
    -- payout row carries the live status; a withdrawal recorded before 029
    -- has no payout to join to and says so rather than claiming 'paid'.
    SELECT e.created_at,
           'withdrawal',
           'php',
           p.amount,
           COALESCE(po.status, 'recorded'),
           COALESCE(po.transaction_id, po.batch_id),
           NULL::uuid,
           'withdrawal:' || e.id::text
      FROM public.ledger_events e
      JOIN public.ledger_postings p
        ON p.event_id = e.id AND p.account = 'member_deposits'
      LEFT JOIN public.payouts po
        ON po.id = (e.payload->>'payout_id')::uuid
     WHERE e.user_id = $1 AND e.kind = 'withdrawal_confirmed'

    UNION ALL

    -- XLM into the vault: the borrower's own transaction, dated when the
    -- engine verified it on Horizon rather than when the row was written.
    SELECT c.locked_at,
           'collateral_lock',
           'xlm',
           c.locked_stroops,
           'confirmed',
           c.lock_tx_hash,
           c.loan_id,
           'lock:' || c.id::text
      FROM public.xlm_collateral c
      JOIN public.loans l ON l.id = c.loan_id
     WHERE l.borrower_id = $1 AND c.locked_at IS NOT NULL

    UNION ALL

    -- XLM out of the vault: released to the borrower on repayment, or seized
    -- on default. locked_stroops is never zeroed on settlement, so it is
    -- still the amount that moved.
    SELECT COALESCE(a.done_at, a.created_at),
           'collateral_' || a.action,
           'xlm',
           c.locked_stroops,
           CASE WHEN a.status = 'done' THEN 'confirmed' ELSE 'queued' END,
           a.tx_hash,
           c.loan_id,
           'action:' || a.id::text
      FROM public.collateral_actions a
      JOIN public.xlm_collateral c ON c.id = a.collateral_id
      JOIN public.loans l ON l.id = c.loan_id
     WHERE l.borrower_id = $1 AND a.action IN ('release', 'seize')

    UNION ALL

    -- pesos lost to a default (030): the member's own deposit taken to cover
    -- their loan, or a guarantor's pledge taken to cover somebody else's. The
    -- XLM leg is already listed above as its own on-chain movement, so it is
    -- excluded here rather than counted twice.
    SELECT r.created_at,
           'deposit_seized',
           'php',
           r.amount,
           'completed',
           NULL,
           r.loan_id,
           'recovery:' || r.id::text
      FROM public.loan_recoveries r
     WHERE r.user_id = $1 AND r.source <> 'borrower_xlm'
";

type MovementRow = (i64, String, String, i64, String, Option<String>, Option<Uuid>, String);

pub async fn list(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(q): Json<TransactionsRequest>,
) -> Result<Json<TransactionsResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    // clamp rather than reject, same as deposits_list: a stale page gets an
    // empty list and an honest total, not a 4xx round-trip.
    let page = q.page.max(1);
    let offset = (page - 1) * PAGE_SIZE;

    let total: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM ({MOVEMENTS}) m"))
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| db_err(e, "transactions count"))?;

    // `id` breaks ties: two movements can share a second (a lock and the
    // disbursement it triggers), and a page boundary that falls between them
    // must not shuffle on every request.
    let rows: Vec<MovementRow> = sqlx::query_as(&format!(
        "SELECT at, kind, asset, amount, status, reference, loan_id, id
           FROM ({MOVEMENTS}) m
          ORDER BY at DESC, id
          LIMIT $2 OFFSET $3"
    ))
    .bind(user_id)
    .bind(PAGE_SIZE)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "transactions page"))?;

    let items = rows
        .into_iter()
        .map(|(at, kind, asset, amount, status, reference, loan_id, id)| TransactionView {
            id, kind, asset, amount, status, reference, loan_id, at,
        })
        .collect();

    let total_pages = if total == 0 { 1 } else { (total + PAGE_SIZE - 1) / PAGE_SIZE };

    Ok(Json(TransactionsResponse {
        items,
        total,
        page,
        page_size: PAGE_SIZE,
        total_pages,
    }))
}
