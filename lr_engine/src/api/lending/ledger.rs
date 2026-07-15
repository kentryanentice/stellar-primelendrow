//! The ONE writer (blueprint §3.1). Every money mutation in the codebase
//! goes through `commit_event` — there is no other path that inserts
//! ledger_events or ledger_postings, so "did we remember to balance / audit
//! / dedupe?" has exactly one answer to check.

use serde_json::Value;
use sqlx::{PgExecutor, Postgres, Transaction};
use uuid::Uuid;

/// >0 debit, <0 credit, centavos.
pub struct Posting {
    pub account: &'static str,
    pub amount: i64,
}

pub struct EventDraft {
    pub kind: &'static str,
    pub user_id: Option<Uuid>,
    pub loan_id: Option<Uuid>,
    pub deposit_id: Option<Uuid>,
    /// Some(_) only for real-money-in (a PayPal capture id, an on-chain tx
    /// hash) — the unique index makes a re-sent callback bounce off schema.
    pub rail_ref: Option<String>,
    pub payload: Value,
    pub actor_id: Option<Uuid>,
}

pub enum LedgerError {
    /// The postings don't sum to zero — a logic bug, caught before the DB.
    Unbalanced(i64),
    /// This rail reference was already credited — treat as already-processed.
    DuplicateRail,
    Db(sqlx::Error),
}

/// Appends the event and its balanced postings inside the caller's
/// transaction, returning the event id. The caller updates projections in
/// the same `tx` and commits — any failure rolls the whole story back.
pub async fn commit_event(
    tx: &mut Transaction<'_, Postgres>,
    draft: EventDraft,
    postings: &[Posting],
) -> Result<i64, LedgerError> {
    // Defense in depth #1: balance in Rust before touching the DB, so a
    // logic bug fails with a clear message, not a deferred-trigger abort.
    let net: i64 = postings.iter().map(|p| p.amount).sum();
    if net != 0 {
        return Err(LedgerError::Unbalanced(net));
    }

    let event_id: i64 = match sqlx::query_scalar(
        "INSERT INTO public.ledger_events
            (kind, user_id, loan_id, deposit_id, rail_ref, payload, actor_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING id",
    )
    .bind(draft.kind)
    .bind(draft.user_id)
    .bind(draft.loan_id)
    .bind(draft.deposit_id)
    .bind(&draft.rail_ref)
    .bind(&draft.payload)
    .bind(draft.actor_id)
    .fetch_one(&mut **tx)
    .await
    {
        Ok(id) => id,
        Err(e) => {
            if e.as_database_error().is_some_and(|d| d.is_unique_violation()) {
                // idx_ledger_events_rail_ref: this money already arrived once.
                return Err(LedgerError::DuplicateRail);
            }
            return Err(LedgerError::Db(e));
        }
    };

    for p in postings {
        // amount = 0 is refused by the table CHECK; skip writing empty rows
        // for events that carry no money (badge moves, consents).
        sqlx::query(
            "INSERT INTO public.ledger_postings (event_id, account, amount) VALUES ($1, $2, $3)",
        )
        .bind(event_id)
        .bind(p.account)
        .bind(p.amount)
        .execute(&mut **tx)
        .await
        .map_err(LedgerError::Db)?;
    }
    // Defense in depth #2: the DEFERRED trigger re-checks balance at COMMIT.
    Ok(event_id)
}

/// SUM of signed postings for an account. For debit-normal accounts (cash,
/// loans_receivable) a positive number is what the pool holds/is owed.
pub async fn account_balance<'e, X: PgExecutor<'e>>(
    executor: X,
    account: &str,
) -> Result<i64, sqlx::Error> {
    // ::BIGINT: SUM(BIGINT) is NUMERIC in Postgres, which won't decode to i64.
    sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0)::BIGINT FROM public.ledger_postings WHERE account = $1",
    )
    .bind(account)
    .fetch_one(executor)
    .await
}
