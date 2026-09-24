//! POST /pool/withdraw — take back 'available' deposit, and only that.
//!
//! The engine, not the UI, is what refuses frozen money: lots wearing
//! 'lent'/'collateral'/'pledged' simply aren't in the FOR UPDATE set this
//! handler consumes. The pool lock closes the race against a concurrent
//! disbursement deciding on the same cash.
//!
//! Rail note (029): this used to record the withdrawal and leave a note for
//! ops to pay by hand. It now rides the same PayPal Payouts rail as loan
//! proceeds (`payout.rs`), in the same three steps, for the same reason:
//!
//!   1. consume the lots and write the promise (`payout_payable`) and the
//!      `payouts` row in ONE transaction, and COMMIT — the row's primary key
//!      is the idempotency key PayPal will be given, so it must exist before
//!      any money can;
//!   2. call PayPal outside any transaction, holding no locks;
//!   3. record what PayPal said.
//!
//! The books therefore move in two beats rather than one. Requesting a
//! withdrawal no longer posts `cash`: the pesos are still in the platform's
//! PayPal balance at that moment, exactly as they are after a disbursement.
//! `cash` falls in `infra::payouts::settle`, against the transfer's own
//! reference, and only when PayPal confirms SUCCESS. Since `free_cash` reads
//! `cash + payout_payable`, a promised withdrawal is immediately unavailable
//! for new lending — the member's money can't be lent out from under them
//! while it is in flight.

use axum::{Extension, Json, http::{HeaderMap, StatusCode}};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::ledger::{EventDraft, LedgerError, Posting, commit_event, free_cash, retained_funds};
use super::lots;
use super::domain;
use super::payout::{self, PayoutView};
use super::policy;
use super::shared::{db_err, ledger_err, validate_centavos};
use crate::api::users::shared::{E, require_verified_user};

#[derive(Deserialize)]
pub struct WithdrawInput {
    /// Whole centavos.
    amount: i64,
    /// One key per withdrawal attempt, made by the client (041). Sending the
    /// same key again — a double click, a retried request — returns the
    /// withdrawal it already made instead of making a second one.
    request_key: Option<Uuid>,
}

#[derive(Serialize)]
pub struct WithdrawResponse {
    /// The transfer itself, in the same shape GET /payouts returns — the UI
    /// tracks a withdrawal exactly the way it tracks loan proceeds.
    pub payout: PayoutView,
    pub message: &'static str,
}

pub async fn withdraw(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(p): Json<WithdrawInput>,
) -> Result<Json<WithdrawResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;
    let amount = validate_centavos(p.amount)?;
    let request_key = p.request_key.ok_or((
        StatusCode::UNPROCESSABLE_ENTITY,
        "Missing withdrawal request key — refresh the page and try again",
    ))?;

    // A key already used: this is the same withdrawal arriving again, so answer
    // with what it already did.
    if let Some(done) = already_requested(&pool, user_id, request_key, amount).await? {
        return Ok(Json(done));
    }

    // Refused before a single lot is touched: a withdrawal we have nowhere to
    // send would consume the member's deposit into a promise that can never
    // be kept.
    let destination = payout::destination(&pool, user_id).await?;

    let mut tx = pool.begin().await.map_err(|e| db_err(e, "begin withdraw"))?;

    // Locks before decisions: pool first (serializes against disburse), then
    // the caller's own lots.
    sqlx::query("SELECT id FROM public.pool_control WHERE id = 1 FOR UPDATE")
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| db_err(e, "pool lock"))?;

    let my_lots = lots::lock_available_for_user(&mut tx, user_id).await?;
    let withdrawable: i64 = my_lots.iter().map(|l| l.amount).sum();
    if withdrawable < amount {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "That exceeds your withdrawable balance — deposits funding active loans stay locked until repayment",
        ));
    }

    // The books are the wall: even a correct lot sum can't overdraw actual
    // cash (loans out are cash gone until repaid).
    // Free cash: earlier withdrawals already promised but not yet paid out are
    // not available to cover this one, even though they haven't left the
    // platform's PayPal balance yet (029). Nor are the platform, reserve and
    // recovery funds — held, not the members' to draw on. A borrower's own
    // waiting proceeds are NOT netted off here: they are exactly the kind of
    // money a withdrawal is for (050).
    let cash = free_cash(&mut *tx)
        .await
        .map_err(|e| db_err(e, "cash balance"))?
        - retained_funds(&mut *tx)
            .await
            .map_err(|e| db_err(e, "retained funds"))?;
    if cash < amount {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "The pool can't cover this right now — most of it is out on loans. Try again after repayments arrive",
        ));
    }

    // Which of these lots are loan proceeds (050). Counted as they are
    // consumed and recorded on the withdrawal event, so that if the payout is
    // refused the refund can hand the proceeds back AS proceeds — otherwise a
    // failed withdrawal would quietly turn borrowed money into pool deposits.
    let proceeds_lots: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM public.deposits WHERE id = ANY($1) AND origin = 'loan_proceeds'",
    )
    .bind(my_lots.iter().map(|l| l.id).collect::<Vec<_>>())
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| db_err(e, "proceeds lots"))?;

    // Consume FIFO: whole lots go, the partial tail shrinks in place. The
    // withdrawal event below is the auditable record of where they went.
    let now = Utc::now().timestamp();
    let mut remaining = amount;
    let mut from_proceeds = 0i64;
    for lot in &my_lots {
        if remaining == 0 {
            break;
        }
        if proceeds_lots.contains(&lot.id) {
            from_proceeds += lot.amount.min(remaining);
        }
        if lot.amount <= remaining {
            sqlx::query("DELETE FROM public.deposits WHERE id = $1")
                .bind(lot.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| db_err(e, "consume lot"))?;
            remaining -= lot.amount;
        } else {
            sqlx::query("UPDATE public.deposits SET amount = amount - $1, updated_at = $2 WHERE id = $3")
                .bind(remaining)
                .bind(now)
                .bind(lot.id)
                .execute(&mut *tx)
                .await
                .map_err(|e| db_err(e, "shrink lot"))?;
            remaining = 0;
        }
    }

    // The destination AND the rail are pinned to the row now, for the same
    // reason: relinking an account later — or an operator flipping
    // PAYOUT_RAIL — must not redirect a transfer that is already in flight.
    // The provider's payout fee comes out of what is sent (043): the member's
    // balance falls by `amount`, they receive `amount - fee`, and the pool's
    // cash falls by exactly `amount` once the provider's own charge is added.
    let rules = policy::active(&mut *tx).await?;
    let fee = domain::payout_fee(amount, rules.params.payment_fees.for_rail(destination.provider));

    let payout_id: Result<Uuid, sqlx::Error> = sqlx::query_scalar(
        "INSERT INTO public.payouts (user_id, loan_id, kind, amount, payer_id, provider, request_key, fee)
         VALUES ($1, NULL, 'deposit_withdrawal', $2, $3, $4, $5, $6)
         RETURNING id",
    )
    .bind(user_id)
    .bind(amount)
    .bind(&destination.account)
    .bind(destination.provider)
    .bind(request_key)
    .bind(fee)
    .fetch_one(&mut *tx)
    .await;
    let payout_id: Uuid = match payout_id {
        Ok(id) => id,
        // The same key landed twice at the same moment and the other request
        // won the unique index: this transaction (and the lots it consumed)
        // rolls back, and the member gets the withdrawal that was made.
        Err(e) if e.as_database_error().is_some_and(|d| d.is_unique_violation()) => {
            drop(tx);
            return already_requested(&pool, user_id, request_key, amount)
                .await?
                .map(Json)
                .ok_or((StatusCode::CONFLICT, "This withdrawal is already being processed"));
        }
        Err(e) => return Err(db_err(e, "insert withdrawal payout")),
    };

    commit_event(
        &mut tx,
        EventDraft {
            kind: "withdrawal_confirmed",
            user_id: Some(user_id),
            loan_id: None,
            deposit_id: None,
            rail_ref: None,
            payload: serde_json::json!({
                "amount": amount, "payout_id": payout_id, "rail": destination.provider,
                "from_proceeds": from_proceeds,
            }),
            actor_id: Some(user_id),
        },
        // The member's deposit claim is gone and the pool now OWES them the
        // pesos. `cash` is untouched until the transfer actually settles.
        &[
            Posting { account: "member_deposits", amount },
            Posting { account: "payout_payable", amount: -amount },
        ],
    )
    .await
    .map_err(|e| ledger_err(e, "withdrawal"))?;

    // Committed BEFORE the network call: the idempotency key has to survive a
    // crash mid-request, or a retry would mint a new one and PayPal would have
    // no way to recognise the duplicate.
    tx.commit().await.map_err(|e| db_err(e, "commit withdraw"))?;

    let (status, message) =
        payout::submit(&pool, payout_id, &destination, amount - fee, "PrimeLendRow withdrawal").await;

    // Refused on the spot — give the money back before answering, so the
    // member sees their balance intact rather than a hole they have to ask
    // about. A no-op for any other status.
    if let Err(e) = refund_if_failed(&pool, payout_id).await {
        // The books are unchanged and the payout row is still terminal, so
        // the sweep in `infra::payouts` will pick this up on its next tick.
        tracing::error!(%payout_id, "withdrawal refund failed, leaving for the sweep: {e}");
    }

    let payout = payout::read_one(&pool, payout_id, user_id).await?;
    tracing::info!(%user_id, %payout_id, amount, status, "withdrawal confirmed");

    Ok(Json(WithdrawResponse { payout, message }))
}

/// The withdrawal a request key already made, if any. The same key with a
/// different amount is a client bug, not a retry, and is refused rather than
/// guessed at.
async fn already_requested(
    pool: &PgPool,
    user_id: Uuid,
    request_key: Uuid,
    amount: i64,
) -> Result<Option<WithdrawResponse>, E> {
    let existing: Option<(Uuid, i64)> = sqlx::query_as(
        "SELECT id, amount FROM public.payouts
          WHERE user_id = $1 AND request_key = $2 AND kind = 'deposit_withdrawal'",
    )
    .bind(user_id)
    .bind(request_key)
    .fetch_optional(pool)
    .await
    .map_err(|e| db_err(e, "withdrawal request key"))?;
    let Some((payout_id, existing_amount)) = existing else {
        return Ok(None);
    };
    if existing_amount != amount {
        return Err((StatusCode::CONFLICT, "That withdrawal request was already used for a different amount"));
    }
    Ok(Some(WithdrawResponse {
        payout: payout::read_one(pool, payout_id, user_id).await?,
        message: "This withdrawal was already requested — here is where it stands",
    }))
}

/// Gives back a withdrawal that never left.
///
/// A failed payout leaves `payout_payable` standing, and for **loan proceeds**
/// that is right: the borrower is still owed their loan, the promise survives,
/// and they can ask for it again. For a **withdrawal** it is wrong, and this is
/// the difference the original rail missed. Requesting a withdrawal consumes
/// the member's deposit lots. If the transfer is then refused, there is nothing
/// left to ask again *with* — the lots are gone, the payable stands forever,
/// and because `free_cash` nets the payable out, the pool loses that peso of
/// lending capacity permanently too. The money goes missing from both sides at
/// once.
///
/// So a refused withdrawal is undone rather than remembered: the deposit comes
/// back as a fresh `available` lot and the postings are reversed, putting the
/// member exactly where they stood before they pressed the button.
///
/// **Idempotent by the same wall as everything else.** The reversal carries
/// `withdrawal_refund:<payout id>` as its `rail_ref`, so the unique index
/// refuses a second refund of the same payout however many times this is
/// called — from the request handler, from the retry sweep, or from both at
/// once. There is no path that pays the money back twice.
///
/// Deliberately takes no view on *why* it failed. A refused payout and a
/// returned one are the same fact from the pool's side: the pesos never
/// reached the member.
pub(crate) async fn refund_if_failed(pool: &PgPool, payout_id: Uuid) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    // Locked first: the request handler and the sweep can both arrive here for
    // the same row, and the loser must see the winner's status change.
    let row: Option<(Uuid, i64, String, String)> = sqlx::query_as(
        "SELECT user_id, amount, kind, status FROM public.payouts
          WHERE id = $1 FOR UPDATE",
    )
    .bind(payout_id)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((user_id, amount, kind, status)) = row else {
        tx.rollback().await?;
        return Ok(());
    };

    // Loan proceeds keep their payable — the borrower is still owed the loan.
    if kind != "deposit_withdrawal" {
        tx.rollback().await?;
        return Ok(());
    }
    // Only terminal, never-arrived states. `unclaimed` is still in flight and
    // PayPal returns it on its own after 30 days, at which point it becomes
    // `returned` and comes back through here.
    if status != "failed" && status != "returned" {
        tx.rollback().await?;
        return Ok(());
    }

    // How much of it was loan proceeds, as the withdrawal recorded when it
    // consumed them (050). Withdrawals made before that carry no figure, and
    // give back as they always did.
    let from_proceeds: i64 = sqlx::query_scalar(
        "SELECT COALESCE((payload->>'from_proceeds')::BIGINT, 0) FROM public.ledger_events
          WHERE kind = 'withdrawal_confirmed' AND payload->>'payout_id' = $1::text
          ORDER BY id LIMIT 1",
    )
    .bind(payout_id)
    .fetch_optional(&mut *tx)
    .await?
    .unwrap_or(0)
    .clamp(0, amount);

    // New lots rather than an attempt to rebuild the ones consumed: withdrawal
    // deletes lots FIFO and shrinks the partial tail, so the originals are not
    // recoverable and pretending otherwise would invent a deposit history that
    // never happened. Two lots at most — the proceeds part goes back AS
    // proceeds, so a failed withdrawal cannot turn borrowed money into pool
    // deposits — and `available`, because that is what it was.
    let refund_lot = |origin: &'static str, lot_amount: i64| {
        sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO public.deposits (user_id, amount, badge, origin) VALUES ($1, $2, 'available', $3)
             RETURNING id",
        )
        .bind(user_id)
        .bind(lot_amount)
        .bind(origin)
    };
    let mut lot_id = None;
    if amount - from_proceeds > 0 {
        lot_id = Some(refund_lot("withdrawal_refund", amount - from_proceeds).fetch_one(&mut *tx).await?);
    }
    if from_proceeds > 0 {
        let proceeds_lot = refund_lot("loan_proceeds", from_proceeds).fetch_one(&mut *tx).await?;
        lot_id = lot_id.or(Some(proceeds_lot));
    }

    let posted = commit_event(
        &mut tx,
        EventDraft {
            kind: "withdrawal_refunded",
            user_id: Some(user_id),
            loan_id: None,
            deposit_id: lot_id,
            rail_ref: Some(format!("withdrawal_refund:{payout_id}")),
            payload: serde_json::json!({
                "amount": amount, "payout_id": payout_id, "reason": status,
                "from_proceeds": from_proceeds,
            }),
            actor_id: None,
        },
        // The exact inverse of what `withdraw` posted: the promise is
        // cancelled and the member's deposit claim is restored.
        &[
            Posting { account: "payout_payable", amount },
            Posting { account: "member_deposits", amount: -amount },
        ],
    )
    .await;

    match posted {
        Ok(_) => {
            tx.commit().await?;
            tracing::info!(%user_id, %payout_id, amount, "withdrawal refunded to deposits");
            Ok(())
        }
        Err(LedgerError::DuplicateRail) => {
            // Already refunded by an earlier call. The lot inserted above rolls
            // back with everything else, so no second deposit survives.
            tx.rollback().await?;
            Ok(())
        }
        Err(LedgerError::Db(e)) => {
            tx.rollback().await?;
            Err(e)
        }
        Err(LedgerError::Unbalanced(net)) => {
            tx.rollback().await?;
            // Not reachable — the two postings above are inverses — but a
            // silent swallow here would be a silent loss of the member's money.
            tracing::error!(%payout_id, net, "withdrawal refund unbalanced");
            Ok(())
        }
    }
}
