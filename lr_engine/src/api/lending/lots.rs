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

use super::domain;
use crate::api::users::shared::E;

#[derive(Clone)]
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

/// Locks and returns the lots wearing `badge` against one loan, oldest first.
/// The badge is the whole authorization story: 'collateral' is the borrower's
/// own money, 'pledged' is a guarantor's, and 'lent' is an uninvolved saver's
/// — a recovery that mixed them up would take the wrong person's deposit.
pub async fn lock_backing_lots(
    tx: &mut Transaction<'_, Postgres>,
    loan_id: Uuid,
    badge: &str,
) -> Result<Vec<Lot>, E> {
    let rows: Vec<(Uuid, Uuid, i64)> = sqlx::query_as(
        "SELECT id, user_id, amount FROM public.deposits
          WHERE backing_loan = $1 AND badge = $2
          ORDER BY created_at, id
          FOR UPDATE",
    )
    .bind(loan_id)
    .bind(badge)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| db_err(e, "lock backing lots"))?;
    Ok(rows.into_iter().map(|(id, user_id, amount)| Lot { id, user_id, amount }).collect())
}

/// Consumes up to `limit` centavos of the given (already locked) lots, FIFO —
/// whole lots deleted, the partial tail shrunk in place, exactly as a
/// withdrawal consumes them. This is the destructive half of a default: the
/// money stops being the member's, so the lot stops existing.
///
/// Returns what was actually taken, per owner, so the caller can post one
/// ledger entry per member rather than one per lot — a guarantor whose pledge
/// spans four lots lost one amount, not four.
pub async fn seize_lots(
    tx: &mut Transaction<'_, Postgres>,
    lots: &[Lot],
    limit: i64,
) -> Result<Vec<(Uuid, i64)>, E> {
    let now = Utc::now().timestamp();
    let mut remaining = limit;
    let mut taken: Vec<(Uuid, i64)> = Vec::new();

    for lot in lots {
        if remaining == 0 {
            break;
        }
        let amount = lot.amount.min(remaining);
        if lot.amount <= remaining {
            sqlx::query("DELETE FROM public.deposits WHERE id = $1")
                .bind(lot.id)
                .execute(&mut **tx)
                .await
                .map_err(|e| db_err(e, "seize lot"))?;
        } else {
            sqlx::query("UPDATE public.deposits SET amount = amount - $1, updated_at = $2 WHERE id = $3")
                .bind(amount)
                .bind(now)
                .bind(lot.id)
                .execute(&mut **tx)
                .await
                .map_err(|e| db_err(e, "shrink seized lot"))?;
        }
        remaining -= amount;
        match taken.iter_mut().find(|(user_id, _)| *user_id == lot.user_id) {
            Some((_, total)) => *total += amount,
            None => taken.push((lot.user_id, amount)),
        }
    }
    Ok(taken)
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
            // Same money, so the child keeps the parent's origin (046).
            let child: Uuid = sqlx::query_scalar(
                "INSERT INTO public.deposits (user_id, amount, badge, backing_loan, parent_lot, origin)
                 SELECT $1, $2, $3, $4, $5, origin FROM public.deposits WHERE id = $5
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

/// Groups locked lots by owner, members in id order (so every pro-rata step
/// is deterministic) and each member's lots in the order they were locked.
fn by_member(lots: Vec<Lot>) -> Vec<(Uuid, Vec<Lot>)> {
    let mut members: Vec<(Uuid, Vec<Lot>)> = Vec::new();
    for lot in lots {
        match members.iter_mut().find(|(m, _)| *m == lot.user_id) {
            Some((_, theirs)) => theirs.push(lot),
            None => members.push((lot.user_id, vec![lot])),
        }
    }
    members.sort_by_key(|(member, _)| *member);
    members
}

/// Funds a disbursement from every depositor at once: each member's available
/// balance goes 'lent' in proportion to their share of all available
/// balances, until the locked slices add up to `amount`. Returns what each
/// member put in.
///
/// Largest-remainder rounding (`domain::apportion_capped`) makes the slices
/// sum to `amount` exactly and never exceed a member's own balance. Within a
/// member, their oldest lots go first, splitting the last one.
///
/// Tolerant of the pool's available balances covering less than `amount`
/// (cash from repayments carries no lots until it is paid back to them), but
/// the cash check in `disburse` is the real liquidity gate — and it no longer
/// counts the platform, reserve and recovery funds, which are held, not lent.
pub async fn freeze_funding_pro_rata(
    tx: &mut Transaction<'_, Postgres>,
    amount: i64,
    backing_loan: Uuid,
) -> Result<Vec<(Uuid, i64)>, E> {
    if amount <= 0 {
        return Ok(Vec::new());
    }
    // A borrower's proceeds waiting to be withdrawn are never lent on: they
    // can be withdrawn at any moment, and `disburse` nets them out of the cash
    // it lends from for the same reason (050).
    let rows: Vec<(Uuid, Uuid, i64)> = sqlx::query_as(
        "SELECT id, user_id, amount FROM public.deposits
          WHERE badge = 'available' AND origin <> 'loan_proceeds'
          ORDER BY created_at, id
          FOR UPDATE",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| db_err(e, "lock pool lots"))?;
    let members = by_member(rows.into_iter().map(|(id, user_id, amount)| Lot { id, user_id, amount }).collect());

    let balances: Vec<i64> = members.iter().map(|(_, lots)| lots.iter().map(|l| l.amount).sum()).collect();
    let slices = domain::apportion_capped(amount, &balances);

    let mut funded = Vec::new();
    for ((member, lots), slice) in members.iter().zip(slices) {
        if slice > 0 {
            rebadge_fifo(tx, lots, slice, "lent", backing_loan).await?;
            funded.push((*member, slice));
        }
    }
    Ok(funded)
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

/// Every member's whole deposit balance (all badges: what they can withdraw,
/// what is lent out, what backs their own loan, what they pledged), members in
/// id order. This is what the depositors' share of interest is divided by.
/// Loan proceeds are left out (050): they are borrowed money the pool never
/// lends on, so they earn the borrower no share of anyone's interest.
///
/// Read without locks on purpose. The balances are a snapshot of the pool at
/// the moment a payment lands, and the interest is credited as brand-new lots
/// rather than by updating existing ones, so a repayment never waits on — or
/// deadlocks against — a withdrawal or a recovery touching the same member.
pub async fn deposit_balances(tx: &mut Transaction<'_, Postgres>) -> Result<Vec<(Uuid, i64)>, E> {
    sqlx::query_as(
        "SELECT user_id, SUM(amount)::BIGINT FROM public.deposits
          WHERE origin <> 'loan_proceeds'
          GROUP BY user_id
         HAVING SUM(amount) > 0
          ORDER BY user_id",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| db_err(e, "deposit balances"))
}

/// What is still locked funding `loan_id`, in total.
pub async fn locked_funding(tx: &mut Transaction<'_, Postgres>, loan_id: Uuid) -> Result<i64, E> {
    sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0)::BIGINT FROM public.deposits
          WHERE backing_loan = $1 AND badge = 'lent'",
    )
    .bind(loan_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| db_err(e, "locked funding"))
}

/// Unlocks `amount` of the 'lent' lots funding `loan_id` as principal comes
/// back, pro-rata across the members funding it — the same shape the money
/// was locked in, so nobody's balance is freed ahead of anyone else's. The
/// caller decides how much (`domain::funding_to_release`); this decides whose.
pub async fn release_funding_pro_rata(
    tx: &mut Transaction<'_, Postgres>,
    loan_id: Uuid,
    amount: i64,
) -> Result<(), E> {
    if amount <= 0 {
        return Ok(());
    }
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
    let members = by_member(rows.into_iter().map(|(id, user_id, amount)| Lot { id, user_id, amount }).collect());

    let lent: Vec<i64> = members.iter().map(|(_, lots)| lots.iter().map(|l| l.amount).sum()).collect();
    let slices = domain::apportion_capped(amount, &lent);
    let mut moves = Vec::new();
    for ((_, lots), slice) in members.iter().zip(slices) {
        moves.extend(plan_release(lots, slice));
    }
    if moves.is_empty() {
        return Ok(());
    }
    let changed: i64 = sqlx::query_scalar(
        "WITH plan AS (
            SELECT * FROM jsonb_to_recordset($1) AS p(id uuid,released bigint,whole boolean)
         ), changed AS (
            UPDATE public.deposits d SET
                amount=CASE WHEN p.whole THEN d.amount ELSE d.amount-p.released END,
                badge=CASE WHEN p.whole THEN 'available' ELSE d.badge END,
                backing_loan=CASE WHEN p.whole THEN NULL ELSE d.backing_loan END,
                updated_at=$2
            FROM plan p WHERE d.id=p.id
            RETURNING d.id,d.user_id,d.origin,p.released,p.whole
         ), children AS (
            INSERT INTO public.deposits (user_id,amount,badge,parent_lot,origin)
            SELECT user_id,released,'available',id,origin FROM changed WHERE NOT whole
            RETURNING id
         ) SELECT count(*)::bigint FROM changed",
    )
    .bind(serde_json::json!(moves))
    .bind(Utc::now().timestamp())
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| db_err(e, "release funding lots"))?;
    if changed != moves.len() as i64 {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Funding lots changed while releasing",
        ));
    }
    Ok(())
}

#[derive(serde::Serialize, Debug, PartialEq)]
struct ReleaseMove {
    id: Uuid,
    released: i64,
    whole: bool,
}

/// Same FIFO choices as individual updates, planned before the bulk write.
fn plan_release(lots: &[Lot], mut amount: i64) -> Vec<ReleaseMove> {
    let mut moves = Vec::new();
    for lot in lots {
        if amount == 0 {
            break;
        }
        let released = amount.min(lot.amount);
        moves.push(ReleaseMove {
            id: lot.id,
            released,
            whole: released == lot.amount,
        });
        amount -= released;
    }
    debug_assert_eq!(amount, 0);
    moves
}

#[cfg(test)]
mod release_tests {
    use super::*;
    #[tokio::test]
    #[ignore = "uses the configured sandbox database; rolls all fixtures back"]
    async fn bulk_release_preserves_balances_origins_and_parentage() {
        dotenvy::dotenv().ok();
        assert_eq!(std::env::var("PAYPAL_ENV").as_deref(), Ok("sandbox"));
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&std::env::var("DATABASE_URL").unwrap())
            .await
            .unwrap();
        let mut tx = pool.begin().await.unwrap();
        let user:Uuid=sqlx::query_scalar("SELECT id FROM public.users WHERE email LIKE 'sim%@example.invalid' AND role='User' ORDER BY email LIMIT 1")
            .fetch_one(&mut *tx).await.unwrap();
        let loan:Uuid=sqlx::query_scalar("INSERT INTO public.loans (borrower_id,product,principal,rate_bps,term_months,policy_version,status) SELECT $1,'deposit_backed',300,200,3,id,'closed' FROM public.policy_versions WHERE active RETURNING id")
            .bind(user).fetch_one(&mut *tx).await.unwrap();
        let first:Uuid=sqlx::query_scalar("INSERT INTO public.deposits(user_id,amount,badge,backing_loan,origin,created_at) VALUES ($1,100,'lent',$2,'deposit',1) RETURNING id")
            .bind(user).bind(loan).fetch_one(&mut *tx).await.unwrap();
        let second:Uuid=sqlx::query_scalar("INSERT INTO public.deposits(user_id,amount,badge,backing_loan,origin,created_at) VALUES ($1,200,'lent',$2,'interest',2) RETURNING id")
            .bind(user).bind(loan).fetch_one(&mut *tx).await.unwrap();
        release_funding_pro_rata(&mut tx, loan, 150).await.unwrap();
        let rows:Vec<(Uuid,i64,String,Option<Uuid>,Option<Uuid>,String)>=sqlx::query_as("SELECT id,amount,badge,backing_loan,parent_lot,origin FROM public.deposits WHERE id=ANY($1) OR parent_lot=$2")
            .bind(vec![first,second]).bind(second).fetch_all(&mut *tx).await.unwrap();
        assert_eq!(rows.iter().map(|r| r.1).sum::<i64>(), 300);
        assert!(rows.iter().any(|r| r.0 == first
            && r.1 == 100
            && r.2 == "available"
            && r.3.is_none()
            && r.5 == "deposit"));
        assert!(rows.iter().any(|r| r.0 == second
            && r.1 == 150
            && r.2 == "lent"
            && r.3 == Some(loan)
            && r.5 == "interest"));
        assert!(rows.iter().any(|r| r.4 == Some(second)
            && r.1 == 50
            && r.2 == "available"
            && r.3.is_none()
            && r.5 == "interest"));
        tx.rollback().await.unwrap();
        pool.close().await;
    }
    #[test]
    fn release_preserves_fifo_and_splits_only_the_tail() {
        let owner = Uuid::from_u128(1);
        let lots = vec![
            Lot {
                id: Uuid::from_u128(2),
                user_id: owner,
                amount: 100,
            },
            Lot {
                id: Uuid::from_u128(3),
                user_id: owner,
                amount: 200,
            },
            Lot {
                id: Uuid::from_u128(4),
                user_id: owner,
                amount: 300,
            },
        ];
        assert_eq!(plan_release(&lots, 0), vec![]);
        assert_eq!(
            plan_release(&lots, 250),
            vec![
                ReleaseMove {
                    id: lots[0].id,
                    released: 100,
                    whole: true
                },
                ReleaseMove {
                    id: lots[1].id,
                    released: 150,
                    whole: false
                }
            ]
        );
        let full = plan_release(&lots, 600);
        assert_eq!(full.iter().map(|m| m.released).sum::<i64>(), 600);
        assert!(full.iter().all(|m| m.whole));
    }
}
