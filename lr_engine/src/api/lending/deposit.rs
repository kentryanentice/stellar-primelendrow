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

    // Three numbers, kept apart, exactly as a repayment keeps them (043, 049):
    //   gross         what the member was charged — their deposit plus the fee
    //   credited      what they asked to deposit, and what their lot receives
    //   provider_fee  what PayPal/Stripe actually kept, as they report it
    //
    // The member paid `gross - credited` on top as the estimated fee. Where the
    // estimate and the provider's reality differ — usually a few pesos either
    // way — the gap is booked to `payment_fee_variance` rather than quietly
    // changing what the member receives, so `cash` still tracks the provider
    // balance to the centavo.
    let gross = captured.centavos;
    let rules = policy::active(pool).await?;
    let provider_fee = captured
        .fee
        .unwrap_or_else(|| domain::receive_fee_estimate(gross, rules.params.payment_fees.for_rail(rail)));
    // `None` on an intent reserved before 049, which charged the round number
    // and credited the net of it. Those keep their original meaning.
    let credited = claimed.applies.unwrap_or(gross - provider_fee);
    let fee_paid = gross - credited;
    let fee_variance = fee_paid - provider_fee;
    if credited <= 0 {
        return Err(intents::refund(pool, claimed, &captured.capture_id, "the provider fee left nothing to credit").await);
    }

    let mut tx = pool.begin().await.map_err(|e| db_err(e, "begin deposit"))?;

    let lot_id: Uuid = sqlx::query_scalar(
        "INSERT INTO public.deposits (user_id, amount, badge, origin) VALUES ($1, $2, 'available', 'deposit')
         RETURNING id",
    )
    .bind(user_id)
    .bind(credited)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| db_err(e, "insert lot"))?;

    // Before the ledger event: the 041 trigger refuses a deposit event that
    // doesn't match a claimed intent carrying this capture reference.
    intents::consume(&mut tx, claimed.id, &captured.capture_id, provider_fee).await?;

    match commit_event(
        &mut tx,
        EventDraft {
            kind: "deposit_confirmed",
            user_id: Some(user_id),
            loan_id: None,
            deposit_id: Some(lot_id),
            rail_ref: Some(captured.capture_id),
            // `amount` stays the gross: it is what the member paid in and what
            // the 041 trigger matches to the intent. `credited` is what they
            // asked to deposit and what the AML limits count (049) — counting
            // the gross there would charge a member's own limit for the
            // provider's fee.
            //
            // Two fees, kept apart exactly as a repayment keeps them. `fee` is
            // what the MEMBER paid on top, `amount - credited`, so the
            // transaction record's "paid · fee · credited" always adds up for
            // the person reading it. `provider_fee` is what PayPal/Stripe
            // actually kept; the gap between them is `payment_fee_variance`,
            // which is the books' concern and not the member's.
            payload: serde_json::json!({
                "rail": rail, "amount": gross, "fee": fee_paid,
                "provider_fee": provider_fee, "credited": credited,
            }),
            actor_id: Some(user_id),
        },
        // `cash` moves by what actually reached the provider balance; the
        // member's balance moves by what they deposited. The variance is the
        // difference between the fee they were charged for and the one the
        // provider really took, and it is what makes the two reconcile.
        &{
            let mut postings = vec![
                Posting { account: "cash", amount: gross - provider_fee },
                Posting { account: "member_deposits", amount: -credited },
            ];
            if fee_variance != 0 {
                postings.push(Posting { account: "payment_fee_variance", amount: -fee_variance });
            }
            postings
        },
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
    tracing::info!(%user_id, %lot_id, gross, provider_fee, credited, fee_variance, rail, "deposit confirmed");

    Ok(DepositResponse {
        lot_id,
        // What landed: the round number the member asked for.
        amount: credited,
        // What they were charged for it, fee included.
        paid: gross,
        fee: fee_paid,
        message: "Deposit received — it's in the pool and withdrawable until it funds a loan",
    })
}
