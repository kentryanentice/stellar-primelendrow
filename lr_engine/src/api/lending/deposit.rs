//! POST /pool/deposit — PHP money-in, over either rail.
//!
//! The client sends nothing but the reference it just paid against — a PayPal
//! order id, or a Stripe Checkout Session id. The engine verifies it
//! server-side (`rails::capture`, provider secrets never leave the backend)
//! and credits exactly what the provider says was collected — the client's
//! screen never decides a centavo. A re-sent reference bounces off the
//! ledger's unique rail_ref (idempotent money-in, Lesson 9).

use axum::{Extension, Json, http::HeaderMap};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::ledger::{EventDraft, LedgerError, Posting, commit_event};
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
    pub amount: i64,
    pub message: &'static str,
}

pub async fn deposit(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(p): Json<DepositInput>,
) -> Result<Json<DepositResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    // Network call happens BEFORE the transaction — no DB locks are ever
    // held across a round-trip to a payment provider.
    let Captured { rail, payment: captured } = rails::capture(&p.payment, user_id).await?;

    credit(&pool, user_id, rail, captured).await.map(Json)
}

/// Turns a verified payment into a deposit lot and the postings behind it.
///
/// Split out from the handler because the Stripe webhook credits through here
/// too: a member who pays and never comes back through the redirect would
/// otherwise have money at the provider and nothing in the pool. Both callers
/// arrive with a payment the provider itself confirmed, and both rely on the
/// same wall — the ledger's unique `rail_ref` — so whichever gets here first
/// wins and the second bounces off the schema.
pub(crate) async fn credit(
    pool: &PgPool,
    user_id: Uuid,
    rail: &'static str,
    captured: rails::CapturedPayment,
) -> Result<DepositResponse, E> {
    let rules = policy::active(pool).await?;

    if captured.centavos < rules.params.min_deposit {
        // The money was really captured; refusing the lot would strand it.
        // This is a display-side floor — enforce it in the UI before order
        // creation, accept anything actually captured here, but log it.
        tracing::warn!(%user_id, centavos = captured.centavos, "deposit below policy minimum accepted");
    }

    let mut tx = pool.begin().await.map_err(|e| db_err(e, "begin deposit"))?;


    let lot_id: Uuid = sqlx::query_scalar(
        "INSERT INTO public.deposits (user_id, amount, badge) VALUES ($1, $2, 'available')
         RETURNING id",
    )
    .bind(user_id)
    .bind(captured.centavos)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| db_err(e, "insert lot"))?;

    let amount = captured.centavos;
    match commit_event(
        &mut tx,
        EventDraft {
            kind: "deposit_confirmed",
            user_id: Some(user_id),
            loan_id: None,
            deposit_id: Some(lot_id),
            rail_ref: Some(captured.capture_id),
            payload: serde_json::json!({ "rail": rail, "amount": amount }),
            actor_id: Some(user_id),
        },
        &[
            Posting { account: "cash", amount },
            Posting { account: "member_deposits", amount: -amount },
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
    tracing::info!(%user_id, %lot_id, amount, rail, "deposit confirmed");

    Ok(DepositResponse {
        lot_id,
        amount,
        message: "Deposit received — it's in the pool and withdrawable until it funds a loan",
    })
}
