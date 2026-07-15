//! Deposit-lot operations: freeze, release, split — always inside the
//! caller's transaction, always FOR UPDATE first (locks before decisions,
//! blueprint §3.3), so two concurrent handlers can never double-spend the
//! same lot.
//!
//! Splitting: when only part of a lot is needed, the original row shrinks
//! and a child row (parent_lot = original) carries the moved amount under
//! its new badge. One badge per lot, and SUM(amount) is conserved inside
//! the transaction. Every move is recorded by the caller as a ledger event
//! (payload only, no postings — the money didn't leave the pool, it just
//! changed badge).

use axum::http::StatusCode;
use chrono::Utc;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::api::users::shared::E;

pub struct Lot {
    pub id: Uuid,
    pub user_id: Uuid,
    pub amount: i64,
}

fn db_err(e: sqlx::Error, ctx: &'static str) -> E {
    tracing::error!("DB {ctx}: {e}");
    (StatusCode::INTERNAL_SERVER_ERROR, "Unable to process request")
}

/// Locks and returns one user's withdrawable lots, oldest first.
pub async fn lock_available_for_user(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
) -> Result<Vec<Lot>, E> {
    let rows: Vec<(Uuid, Uuid, i64)> = sqlx::query_as(
        "SELECT id, user_id, amount FROM public.deposits
          WHERE user_id = $1 AND badge = 'available'
          ORDER BY created_at, id
          FOR UPDATE",
    )
    .bind(user_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| db_err(e, "lock user lots"))?;
    Ok(rows.into_iter().map(|(id, user_id, amount)| Lot { id, user_id, amount }).collect())
}

/// Re-badges `amount` centavos out of the given (already locked) lots FIFO,
/// splitting the last lot if only part of it is needed. Returns the lot ids
/// now wearing `badge`. Caller must have verified the lots sum to >= amount.
async fn rebadge_fifo(
    tx: &mut Transaction<'_, Postgres>,
    lots: &[Lot],
    mut amount: i64,
    badge: &'static str,
    backing_loan: Uuid,
) -> Result<Vec<Uuid>, E> {
    let now = Utc::now().timestamp();
    let mut moved = Vec::new();
    for lot in lots {
        if amount == 0 {
            break;
        }
        if lot.amount <= amount {
            sqlx::query(
                "UPDATE public.deposits
                    SET badge = $1, backing_loan = $2, updated_at = $3
                  WHERE id = $4",
            )
            .bind(badge)
            .bind(backing_loan)
            .bind(now)
            .bind(lot.id)
            .execute(&mut **tx)
            .await
            .map_err(|e| db_err(e, "rebadge lot"))?;
            moved.push(lot.id);
            amount -= lot.amount;
        } else {
            // Partial: shrink the free lot, split the frozen part into a child.
            sqlx::query("UPDATE public.deposits SET amount = amount - $1, updated_at = $2 WHERE id = $3")
                .bind(amount)
                .bind(now)
                .bind(lot.id)
                .execute(&mut **tx)
                .await
                .map_err(|e| db_err(e, "shrink lot"))?;
            let child: Uuid = sqlx::query_scalar(
                "INSERT INTO public.deposits (user_id, amount, badge, backing_loan, parent_lot)
                 VALUES ($1, $2, $3, $4, $5)
                 RETURNING id",
            )
            .bind(lot.user_id)
            .bind(amount)
            .bind(badge)
            .bind(backing_loan)
            .bind(lot.id)
            .fetch_one(&mut **tx)
            .await
            .map_err(|e| db_err(e, "split lot"))?;
            moved.push(child);
            amount = 0;
        }
    }
    debug_assert_eq!(amount, 0);
    Ok(moved)
}

/// Freezes `amount` of ONE user's available deposits under `badge`
/// ('collateral' for their own loan, 'pledged' for a vouch). Errors with a
/// clear message when they don't have that much withdrawable.
pub async fn freeze_user_lots(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    amount: i64,
    badge: &'static str,
    backing_loan: Uuid,
) -> Result<Vec<Uuid>, E> {
    let lots = lock_available_for_user(tx, user_id).await?;
    let total: i64 = lots.iter().map(|l| l.amount).sum();
    if total < amount {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "Not enough withdrawable deposit to back this",
        ));
    }
    rebadge_fifo(tx, &lots, amount, badge, backing_loan).await
}

/// Marks pool-wide available lots 'lent' to fund a disbursement, oldest
/// first across ALL depositors — this is the "your deposit is locked while
/// it funds a loan" the depositor sees. Tolerant of covering less than
/// `amount`: the cash check in the disburse path is the real liquidity gate
/// (retained earnings are cash without lots).
pub async fn freeze_funding_lots(
    tx: &mut Transaction<'_, Postgres>,
    amount: i64,
    backing_loan: Uuid,
) -> Result<(), E> {
    let rows: Vec<(Uuid, Uuid, i64)> = sqlx::query_as(
        "SELECT id, user_id, amount FROM public.deposits
          WHERE badge = 'available'
          ORDER BY created_at, id
          FOR UPDATE",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| db_err(e, "lock pool lots"))?;
    let lots: Vec<Lot> = rows.into_iter().map(|(id, user_id, amount)| Lot { id, user_id, amount }).collect();
    let total: i64 = lots.iter().map(|l| l.amount).sum();
    rebadge_fifo(tx, &lots, amount.min(total), "lent", backing_loan).await?;
    Ok(())
}

/// Flips every lot backing `loan_id` that wears one of `badges` back to
/// 'available' — collateral home at close, pledges home on release.
pub async fn release_loan_lots(
    tx: &mut Transaction<'_, Postgres>,
    loan_id: Uuid,
    badges: &[&str],
) -> Result<(), E> {
    let now = Utc::now().timestamp();
    sqlx::query(
        "UPDATE public.deposits
            SET badge = 'available', backing_loan = NULL, updated_at = $1
          WHERE backing_loan = $2 AND badge = ANY($3)",
    )
    .bind(now)
    .bind(loan_id)
    .bind(badges)
    .execute(&mut **tx)
    .await
    .map_err(|e| db_err(e, "release loan lots"))?;
    Ok(())
}

/// Unlocks up to `amount` of the 'lent' lots funding `loan_id` (called as
/// principal repays: the pool got cash back, so that much deposit is
/// withdrawable again). FIFO with a split for the partial tail.
pub async fn release_funding_lots(
    tx: &mut Transaction<'_, Postgres>,
    loan_id: Uuid,
    mut amount: i64,
) -> Result<(), E> {
    let now = Utc::now().timestamp();
    let rows: Vec<(Uuid, Uuid, i64)> = sqlx::query_as(
        "SELECT id, user_id, amount FROM public.deposits
          WHERE backing_loan = $1 AND badge = 'lent'
          ORDER BY created_at, id
          FOR UPDATE",
    )
    .bind(loan_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| db_err(e, "lock lent lots"))?;

    for (id, user_id, lot_amount) in rows {
        if amount == 0 {
            break;
        }
        if lot_amount <= amount {
            sqlx::query(
                "UPDATE public.deposits
                    SET badge = 'available', backing_loan = NULL, updated_at = $1
                  WHERE id = $2",
            )
            .bind(now)
            .bind(id)
            .execute(&mut **tx)
            .await
            .map_err(|e| db_err(e, "release lent lot"))?;
            amount -= lot_amount;
        } else {
            sqlx::query("UPDATE public.deposits SET amount = amount - $1, updated_at = $2 WHERE id = $3")
                .bind(amount)
                .bind(now)
                .bind(id)
                .execute(&mut **tx)
                .await
                .map_err(|e| db_err(e, "shrink lent lot"))?;
            sqlx::query(
                "INSERT INTO public.deposits (user_id, amount, badge, parent_lot)
                 VALUES ($1, $2, 'available', $3)",
            )
            .bind(user_id)
            .bind(amount)
            .bind(id)
            .execute(&mut **tx)
            .await
            .map_err(|e| db_err(e, "split lent lot"))?;
            amount = 0;
        }
    }
    Ok(())
}
