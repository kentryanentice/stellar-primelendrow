//! POST /pool/deposit — PHP money-in, over either rail.
//!
//! The client sends nothing but the reference it just paid against — a PayPal
//! order id, or a Stripe Checkout Session id. The engine verifies it
//! server-side (`rails::capture`, provider secrets never leave the backend)
//! and credits exactly what the provider says was collected — the client's
//! screen never decides a centavo. The reference must belong to a deposit the
//! engine started for this member, within their AML limits, for exactly the
//! amount collected (041). A re-sent reference is refused as already
//! processed, and the ledger's unique rail_ref backs that up (Lesson 9).

use axum::{Extension, Json, http::HeaderMap};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::ledger::{EventDraft, LedgerError, Posting, commit_event};
use super::domain;
use super::intents::{self, Claim, Claimed};
use super::policy;
use super::rails::{self, Captured, PaymentRef};
use super::shared::{db_err, ledger_err};
use crate::api::users::shared::{E, require_verified_user};

#[derive(Deserialize)]
pub struct DepositInput {
    /// Flattened, so `{"order_id": "…"}` — what the PayPal client has always
    /// sent — still parses unchanged, and `{"session_id": "…"}` is the Stripe
    /// equivalent.
    #[serde(flatten)]
    payment: PaymentRef,
}

#[derive(Serialize)]
pub struct DepositResponse {
    pub lot_id: Uuid,
    /// What was credited: `paid - fee`.
    pub amount: i64,
    /// What the member paid in.
    pub paid: i64,
    /// What the payment provider kept (043).
    pub fee: i64,
    pub message: &'static str,
}

pub async fn deposit(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(p): Json<DepositInput>,
) -> Result<Json<DepositResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    // The payment must be a deposit the engine started for this member (041).
    // Claimed before capture, so a PayPal order that doesn't qualify is refused
    // without being charged.
    let claimed = match intents::claim(&pool, user_id, &p.payment, "deposit").await? {
        Claim::Proceed(claimed) => claimed,
        Claim::Stale(claimed) => return Err(intents::refund_stale(&pool, user_id, &p.payment, &claimed).await),
    };

    // Network call happens BEFORE the transaction — no DB locks are ever
    // held across a round-trip to a payment provider.
    let captured = match rails::capture(&p.payment, user_id).await {
        Ok(captured) => captured,
        Err(e) => {
            intents::release_claim(&pool, claimed.id).await;
            return Err(e);
        }
    };

    credit(&pool, user_id, &claimed, captured).await.map(Json)
}

/// Turns a verified, claimed payment into a deposit lot and the postings
/// behind it.
///
/// Split out from the handler because the Stripe webhook credits through here
/// too: a member who pays and never comes back through the redirect would
/// otherwise have money at the provider and nothing in the pool. Both callers
/// claim the intent first and both mark it consumed inside this transaction,
/// so whichever gets here first wins and the second is refused.
pub(crate) async fn credit(
    pool: &PgPool,
    user_id: Uuid,
    claimed: &Claimed,
    captured: Captured,
) -> Result<DepositResponse, E> {
    let Captured { rail, payment: captured } = captured;

    // The provider charged what the engine asked for, or the payment is given
    // back — never credited at a different amount than the limits allowed.
    if captured.centavos != claimed.amount {
        return Err(intents::refund(pool, claimed, &captured.capture_id, "deposit amount did not match its intent").await);
    }

    // The member is credited what actually reached the provider balance: the
    // gross less the fee the provider reports keeping (043), or the policy
    // estimate when it reports none. `cash` and the member's lot then agree
    // with the PayPal/Stripe balance to the centavo.
    let gross = captured.centavos;
    let rules = policy::active(pool).await?;
    let fee = captured
        .fee
        .unwrap_or_else(|| domain::receive_fee_estimate(gross, rules.params.payment_fees.for_rail(rail)));
    let net = gross - fee;
    if net <= 0 {
        return Err(intents::refund(pool, claimed, &captured.capture_id, "the provider fee left nothing to credit").await);
    }

    let mut tx = pool.begin().await.map_err(|e| db_err(e, "begin deposit"))?;

    let lot_id: Uuid = sqlx::query_scalar(
        "INSERT INTO public.deposits (user_id, amount, badge) VALUES ($1, $2, 'available')
         RETURNING id",
    )
    .bind(user_id)
    .bind(net)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| db_err(e, "insert lot"))?;

    // Before the ledger event: the 041 trigger refuses a deposit event that
    // doesn't match a claimed intent carrying this capture reference.
    intents::consume(&mut tx, claimed.id, &captured.capture_id, fee).await?;

    match commit_event(
        &mut tx,
        EventDraft {
            kind: "deposit_confirmed",
            user_id: Some(user_id),
            loan_id: None,
            deposit_id: Some(lot_id),
            rail_ref: Some(captured.capture_id),
            // `amount` stays the gross: it is what the member paid in, what the
            // AML limits count, and what the 041 trigger matches to the intent.
            payload: serde_json::json!({ "rail": rail, "amount": gross, "fee": fee, "net": net }),
            actor_id: Some(user_id),
        },
        &[
            Posting { account: "cash", amount: net },
            Posting { account: "member_deposits", amount: -net },
        ],
    )
    .await
    {
        Ok(_) => {}
        Err(LedgerError::DuplicateRail) => {
            // Whole transaction (including the freshly inserted lot) rolls
            // back — the first arrival of this capture already made the lot.
            return Err(ledger_err(LedgerError::DuplicateRail, "deposit"));
        }
        Err(e) => return Err(ledger_err(e, "deposit")),
    }

    tx.commit().await.map_err(|e| db_err(e, "commit deposit"))?;
    tracing::info!(%user_id, %lot_id, gross, fee, net, rail, "deposit confirmed");

    Ok(DepositResponse {
        lot_id,
        amount: net,
        paid: gross,
        fee,
        message: "Deposit received — it's in the pool and withdrawable until it funds a loan",
    })
}
