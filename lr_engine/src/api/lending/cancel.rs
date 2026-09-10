//! POST /loans/cancel — the borrower withdraws their own application.
//!
//! Only a `pending` loan can be withdrawn, and that word carries the whole
//! safety argument: a pending loan has not funded. No pesos left the pool, no
//! schedule is running, `loans_receivable` has never been touched for it. So
//! cancelling moves no money and posts no balanced pair — it hands back what
//! the application had frozen and closes the row.
//!
//! What it hands back is the borrower's own deposit, held under the
//! 'collateral' badge since they applied. Before this existed there was no way
//! to get it: an application someone thought better of sat holding their money,
//! and the one-open-loan rule in `apply` meant they could not apply for
//! anything else either. The only exit was to let it be declined by somebody
//! else, or to default a loan that had never been disbursed.
//!
//! **Two refusals, and they are not the same kind of refusal.**
//!
//! *A guarantor has already accepted.* Refused, by choice. Accepting freezes
//! that guarantor's own deposit on the strength of this specific loan, at a
//! moment they chose. Letting the borrower dissolve that unilaterally would
//! make a pledge something that can be requested and dropped at will — the
//! guarantor would watch their money move twice on somebody else's whim. The
//! borrower's way out from there is the one that already exists: ask the
//! guarantors to decline, and the loan closes itself (`guarantors`, the
//! decline-all path). An 'invited' row is different — nobody has committed
//! anything yet — so those are simply withdrawn.
//!
//! *The coins are already in the vault.* Refused, because it is not this
//! endpoint's to do. Getting them out is a signed on-chain release
//! (`mark_repaid` then `release`), and claiming a cancellation here while the
//! coins sat in the contract would be the database asserting something the
//! chain had not agreed to — the exact mistake migration 030 corrected
//! elsewhere.
//!
//! No credit consequence. Withdrawing an application you never received money
//! for is not a thing to be marked down for, and the score moves on real
//! behaviour (D5) — a default is behaviour, changing your mind is not.

use axum::{Extension, Json, http::{HeaderMap, StatusCode}};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::ledger::{EventDraft, commit_event};
use super::lots;
use super::shared::{db_err, ledger_err};
use crate::api::users::shared::{E, require_verified_user};

#[derive(Deserialize)]
pub struct CancelInput {
    loan_id: Uuid,
}

#[derive(Serialize)]
pub struct CancelResponse {
    pub loan_id: Uuid,
    /// Centavos of the borrower's own deposit that went back to withdrawable.
    pub released: i64,
    /// Invitations withdrawn — guarantors who were asked but hadn't answered.
    pub invitations_withdrawn: u64,
    pub message: &'static str,
}

pub async fn cancel(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(p): Json<CancelInput>,
) -> Result<Json<CancelResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    let mut tx = pool.begin().await.map_err(|e| db_err(e, "begin cancel"))?;

    // The loan row serializes this against a guarantor accepting at the same
    // moment: whichever transaction takes the lock first decides, and the other
    // sees the result rather than acting on a stale read. Without it a pledge
    // could be accepted between the check below and the cancellation.
    let loan: Option<(Uuid, String, i64)> = sqlx::query_as(
        "SELECT borrower_id, status, principal FROM public.loans
          WHERE id = $1 FOR UPDATE",
    )
    .bind(p.loan_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| db_err(e, "lock loan"))?;
    let (borrower_id, status, principal) =
        loan.ok_or((StatusCode::NOT_FOUND, "No such loan"))?;
    if borrower_id != user_id {
        return Err((StatusCode::NOT_FOUND, "No such loan"));
    }
    if status != "pending" {
        return Err((
            StatusCode::CONFLICT,
            "Only an application that hasn't funded yet can be cancelled",
        ));
    }

    // Somebody has already put their own money behind this. Not ours to undo.
    let accepted: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM public.loan_guarantors
          WHERE loan_id = $1 AND status = 'accepted'",
    )
    .bind(p.loan_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| db_err(e, "accepted pledges"))?;
    if accepted > 0 {
        return Err((
            StatusCode::CONFLICT,
            "A guarantor has already backed this loan — ask them to decline it instead, and it closes itself",
        ));
    }

    // Coins in the vault leave by a signed release, never by a database write.
    let position: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT id, status FROM public.xlm_collateral WHERE loan_id = $1",
    )
    .bind(p.loan_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| db_err(e, "collateral position"))?;
    if position.as_ref().is_some_and(|(_, s)| s == "locked") {
        return Err((
            StatusCode::CONFLICT,
            "Your XLM is already locked in the vault — it has to be released on-chain, so this application can't just be cancelled",
        ));
    }

    let now = Utc::now().timestamp();

    // What the application was holding, measured before it is handed back so
    // the response can say how much moved.
    let released: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0)::BIGINT FROM public.deposits
          WHERE backing_loan = $1 AND badge = 'collateral'",
    )
    .bind(p.loan_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| db_err(e, "frozen cover"))?;

    // Only 'collateral' — the borrower's own. 'pledged' is deliberately absent:
    // there is nothing pledged to release (no acceptance got this far), and
    // naming it here would be code that quietly worked if that ever changed.
    lots::release_loan_lots(&mut tx, p.loan_id, &["collateral"]).await?;

    // Withdrawn, not declined: the borrower took the question back before it
    // was answered, and 'declined' is the guarantor's word, not theirs (035).
    let invitations_withdrawn = sqlx::query(
        "UPDATE public.loan_guarantors SET status = 'cancelled', updated_at = $1
          WHERE loan_id = $2 AND status = 'invited'",
    )
    .bind(now)
    .bind(p.loan_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| db_err(e, "withdraw invitations"))?
    .rows_affected();

    // A position that was never locked did not get 'released' — nothing was
    // ever held. Refused above if it were locked, so this only ever touches a
    // 'pending' one.
    if let Some((collateral_id, _)) = position {
        sqlx::query(
            "UPDATE public.xlm_collateral SET status = 'cancelled', updated_at = $1
              WHERE id = $2 AND status = 'pending'",
        )
        .bind(now)
        .bind(collateral_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| db_err(e, "cancel collateral position"))?;
    }

    sqlx::query(
        "UPDATE public.loans SET status = 'cancelled', closed_at = $1, updated_at = $1
          WHERE id = $2",
    )
    .bind(now)
    .bind(p.loan_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| db_err(e, "cancel loan"))?;

    // No postings. A pending loan never posted anything to undo — `disburse`
    // is what raises `loans_receivable`, and it never ran. The event is the
    // record that the application existed and who ended it.
    commit_event(
        &mut tx,
        EventDraft {
            kind: "loan_cancelled",
            user_id: Some(user_id),
            loan_id: Some(p.loan_id),
            deposit_id: None,
            rail_ref: None,
            payload: serde_json::json!({
                "principal": principal,
                "released": released,
                "invitations_withdrawn": invitations_withdrawn,
            }),
            actor_id: Some(user_id),
        },
        &[],
    )
    .await
    .map_err(|e| ledger_err(e, "loan_cancelled"))?;

    tx.commit().await.map_err(|e| db_err(e, "commit cancel"))?;
    tracing::info!(%user_id, loan = %p.loan_id, released, invitations_withdrawn, "loan application cancelled");

    Ok(Json(CancelResponse {
        loan_id: p.loan_id,
        released,
        invitations_withdrawn,
        message: if released > 0 {
            "Application cancelled — your deposit is withdrawable again"
        } else {
            "Application cancelled"
        },
    }))
}
