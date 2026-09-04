//! POST /lending/admin/loans/default — declare a loan defaulted, admin-only.
//!
//! Two things happen here and they are deliberately separable:
//!
//!   * the **declaration** — the loan's outcome, the schedule marked, the
//!     credit consequence, and (for an XLM loan) `mark_defaulted` + `seize`
//!     queued for the vault admin to sign. This is the part an administrator
//!     decides.
//!   * the **recovery** — `recovery::advance`, which takes the borrower's own
//!     deposits immediately and then stops at a locked collateral position,
//!     because how much debt the coins cover is decided on-chain and not here.
//!
//! Nothing about the on-chain half is claimed until the chain says so: the
//! position stays `locked` and the queued actions stay `queued` until
//! `actions::confirm` verifies a transaction hash against Horizon. An engine
//! that flipped the row to `seized` on the admin's say-so would be asserting
//! something only the network can settle — the same mistake the release path
//! made before 030.
//!
//! There is no automatic overdue sweep behind this. Declaring a default is an
//! administrator's judgement about a borrower, and the SOW's demonstration
//! needs it on demand; a clock that did it unattended would be a different
//! feature with a different risk.

use axum::{Extension, Json, http::{HeaderMap, StatusCode}};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::ledger::{EventDraft, commit_event};
use super::recovery;
use super::shared::{db_err, ledger_err};
use crate::api::users::shared::{E, require_admin};

/// A default is the worst thing a borrower's record can carry, so it costs
/// more than a clean repayment earns (+5, `repay::SCORE_BUMP_ON_CLOSE`).
const SCORE_PENALTY_ON_DEFAULT: i16 = 25;

#[derive(Deserialize)]
pub struct DefaultInput {
    loan_id: Uuid,
    /// Free text for the audit trail — why this loan was called. Optional; an
    /// empty reason is recorded as such rather than invented.
    #[serde(default)]
    reason: String,
}

#[derive(Serialize)]
pub struct DefaultResponse {
    pub loan_id: Uuid,
    /// Centavos still uncovered after everything recoverable right now.
    pub shortfall: i64,
    /// True when a locked XLM position is holding the waterfall open: the
    /// admin has to sign `mark_defaulted` then `seize` before guarantors are
    /// charged.
    pub awaiting_seizure: bool,
    pub message: &'static str,
}

pub async fn declare(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(p): Json<DefaultInput>,
) -> Result<Json<DefaultResponse>, E> {
    let admin_id = require_admin(&pool, &headers).await?;

    let mut tx = pool.begin().await.map_err(|e| db_err(e, "begin default"))?;

    // The loan row is the serialization point: two admins clicking at once
    // produce one default, and the loser sees the conflict.
    let loan: Option<(Uuid, String, i64, String)> = sqlx::query_as(
        "SELECT borrower_id, product, principal_outstanding, status
           FROM public.loans WHERE id = $1 FOR UPDATE",
    )
    .bind(p.loan_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| db_err(e, "lock loan"))?;
    let (borrower_id, product, outstanding, status) =
        loan.ok_or((StatusCode::NOT_FOUND, "No such loan"))?;
    if status != "active" {
        return Err((
            StatusCode::CONFLICT,
            "Only a disbursed, running loan can be defaulted",
        ));
    }

    let now = Utc::now().timestamp();
    sqlx::query(
        "UPDATE public.loans
            SET status = 'defaulted', defaulted_at = $1, updated_at = $1
          WHERE id = $2",
    )
    .bind(now)
    .bind(p.loan_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| db_err(e, "mark loan defaulted"))?;

    // Everything still owed goes with it. A 'paid' row stays paid — the
    // borrower did pay it, and a default doesn't rewrite that.
    sqlx::query(
        "UPDATE public.loan_schedule SET status = 'defaulted'
          WHERE loan_id = $1 AND status <> 'paid'",
    )
    .bind(p.loan_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| db_err(e, "default schedule"))?;

    // The outcome, before any money moves. No postings: a declaration isn't a
    // transfer, and every peso that moves because of it is posted by
    // `recovery` under its own step.
    let reason = p.reason.trim().chars().take(200).collect::<String>();
    commit_event(
        &mut tx,
        EventDraft {
            kind: "loan_defaulted",
            user_id: Some(borrower_id),
            loan_id: Some(p.loan_id),
            deposit_id: None,
            rail_ref: None,
            payload: serde_json::json!({
                "product": product,
                "outstanding": outstanding,
                "reason": if reason.is_empty() { serde_json::Value::Null } else { reason.clone().into() },
            }),
            actor_id: Some(admin_id),
        },
        &[],
    )
    .await
    .map_err(|e| ledger_err(e, "loan_defaulted"))?;

    // Track record moves on real behaviour (D5), the mirror of the bump a
    // full repayment earns — and logged the same way, so a score can always
    // be explained from its own history.
    let old_score: Option<i16> = sqlx::query_scalar(
        "SELECT score FROM public.credit_scores WHERE user_id = $1 FOR UPDATE",
    )
    .bind(borrower_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| db_err(e, "score read"))?;
    if let Some(old_score) = old_score {
        let new_score = (old_score - SCORE_PENALTY_ON_DEFAULT).max(0);
        if new_score != old_score {
            sqlx::query("UPDATE public.credit_scores SET score = $1, updated_at = $2 WHERE user_id = $3")
                .bind(new_score)
                .bind(now)
                .bind(borrower_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| db_err(e, "penalize score"))?;
            sqlx::query(
                "INSERT INTO public.credit_score_log (user_id, old_score, new_score, actor_id, reason)
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(borrower_id)
            .bind(old_score)
            .bind(new_score)
            .bind(admin_id)
            .bind(format!("loan {} defaulted", p.loan_id))
            .execute(&mut *tx)
            .await
            .map_err(|e| db_err(e, "score log"))?;
        }
    }

    // The on-chain half, queued in contract order: the vault refuses `seize`
    // unless a default was recorded against the position first, so the pair
    // goes in that way round and the queue's id order keeps it.
    let position: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM public.xlm_collateral WHERE loan_id = $1 AND status = 'locked'",
    )
    .bind(p.loan_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| db_err(e, "collateral position"))?;
    if let Some(collateral_id) = position {
        sqlx::query(
            "INSERT INTO public.collateral_actions (collateral_id, action, actor_id)
             VALUES ($1, 'mark_defaulted', $2), ($1, 'seize', $2)",
        )
        .bind(collateral_id)
        .bind(admin_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| db_err(e, "queue seizure"))?;
    }

    let progress = recovery::advance(&mut tx, p.loan_id, admin_id).await?;

    tx.commit().await.map_err(|e| db_err(e, "commit default"))?;
    tracing::info!(
        %admin_id, loan = %p.loan_id, outstanding,
        shortfall = progress.shortfall, awaiting = progress.awaiting_seizure,
        "loan defaulted"
    );

    Ok(Json(DefaultResponse {
        loan_id: p.loan_id,
        shortfall: progress.shortfall,
        awaiting_seizure: progress.awaiting_seizure,
        message: if progress.awaiting_seizure {
            "Default recorded. Sign the queued vault movements to seize the collateral — guarantors are charged only for what the coins don't cover"
        } else {
            "Default recorded and recovery settled"
        },
    }))
}
