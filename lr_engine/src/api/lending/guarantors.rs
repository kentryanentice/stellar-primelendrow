//! Guarantor endpoints: see your invitations, accept (freezing your pledge)
//! or decline. Accepting is a D1 consent — the exact peso amount that
//! freezes, recorded as an event you can't later claim you didn't see —
//! and the loan disburses automatically the moment accepted pledges cover
//! the principal.

use axum::{Extension, Json, http::{HeaderMap, StatusCode}};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::ledger::{EventDraft, commit_event};
use super::lots;
use super::shared::{db_err, ledger_err, try_disburse};
use crate::api::users::shared::{E, require_verified_user};

#[derive(Serialize)]
pub struct InviteView {
    pub id: Uuid,
    pub loan_id: Uuid,
    pub borrower: String,
    pub product: String,
    pub amount: i64,
    pub rate_bps: i32,
    pub term_months: i16,
    pub pledge_amount: i64,
    pub status: String,
    pub created_at: i64,
}

#[derive(Serialize)]
pub struct InvitesResponse {
    pub invites: Vec<InviteView>,
}

/// Everything I've been asked to guarantee — pending invites first, then the
/// history (accepted/released/seized), so a guarantor can always see what
/// they're on the hook for.
pub async fn invites(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
) -> Result<Json<InvitesResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    type InviteRow = (Uuid, Uuid, String, String, i64, i32, i16, i64, String, i64);
    let rows: Vec<InviteRow> = sqlx::query_as(
        "SELECT g.id, g.loan_id, u.username, l.product, l.principal, l.rate_bps,
                l.term_months, g.pledge_amount, g.status, g.created_at
           FROM public.loan_guarantors g
           JOIN public.loans l ON l.id = g.loan_id
           JOIN public.users u ON u.id = l.borrower_id
          WHERE g.guarantor_id = $1
          ORDER BY (g.status = 'invited') DESC, g.created_at DESC",
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "invites"))?;

    Ok(Json(InvitesResponse {
        invites: rows
            .into_iter()
            .map(|(id, loan_id, borrower, product, amount, rate_bps, term_months, pledge_amount, status, created_at)| InviteView {
                id, loan_id, borrower, product, amount, rate_bps, term_months, pledge_amount, status, created_at,
            })
            .collect(),
    }))
}

#[derive(Deserialize)]
pub struct RespondInput {
    invite_id: Uuid,
    accept: bool,
}

#[derive(Serialize)]
pub struct RespondResponse {
    pub status: &'static str,
    pub loan_status: String,
    pub message: &'static str,
}

pub async fn respond(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(p): Json<RespondInput>,
) -> Result<Json<RespondResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    let mut tx = pool.begin().await.map_err(|e| db_err(e, "begin respond"))?;

    // Lock my invite row (mine alone), then the loan — every responder takes
    // the shared loan lock last, so two concurrent responses serialize on it
    // without deadlocking.
    let invite: Option<(Uuid, i64, String)> = sqlx::query_as(
        "SELECT g.loan_id, g.pledge_amount, g.status
           FROM public.loan_guarantors g
          WHERE g.id = $1 AND g.guarantor_id = $2
          FOR UPDATE",
    )
    .bind(p.invite_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| db_err(e, "lock invite"))?;
    let (loan_id, pledge_amount, invite_status) =
        invite.ok_or((StatusCode::NOT_FOUND, "Invitation not found"))?;
    if invite_status != "invited" {
        return Err((StatusCode::CONFLICT, "You've already responded to this invitation"));
    }

    let loan: Option<(Uuid, i64, i32, i16, String, i64)> = sqlx::query_as(
        "SELECT borrower_id, principal, rate_bps, term_months, status,
                borrower_cover_centavos
           FROM public.loans WHERE id = $1 FOR UPDATE",
    )
    .bind(loan_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| db_err(e, "lock loan"))?;
    let (borrower_id, principal, rate_bps, term_months, loan_status, borrower_cover) =
        loan.ok_or((StatusCode::NOT_FOUND, "Loan not found"))?;
    if loan_status != "pending" {
        return Err((StatusCode::CONFLICT, "This loan is no longer waiting on guarantors"));
    }

    // What the guarantors are actually on the hook for: the principal less the
    // half (or more) the borrower carries from their own deposit and coins.
    // Loans issued before the 50% rule carry a cover of 0, so this is the full
    // principal for them and they keep the rule they were issued under.
    let guarantor_gap = principal - borrower_cover;

    let now = Utc::now().timestamp();

    if !p.accept {
        sqlx::query("UPDATE public.loan_guarantors SET status = 'declined', updated_at = $1 WHERE id = $2")
            .bind(now)
            .bind(p.invite_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_err(e, "decline invite"))?;

        // If what's left (accepted + still-invited) can no longer cover the
        // principal, the application is dead — free every pledge already
        // frozen rather than holding people's money for a loan that can't fund.
        let remaining: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(pledge_amount), 0)::BIGINT FROM public.loan_guarantors
              WHERE loan_id = $1 AND status IN ('invited','accepted')",
        )
        .bind(loan_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| db_err(e, "remaining pledges"))?;

        let mut loan_status_out = "pending".to_string();
        if remaining < guarantor_gap {
            // Everything frozen for a loan that will never fund goes home:
            // the co-guarantors' pledges, and — since the 50% rule — the
            // borrower's own deposit leg, which froze at apply time. Leaving
            // 'collateral' out here would strand the borrower's own money
            // behind a dead application.
            lots::release_loan_lots(&mut tx, loan_id, &["pledged", "collateral"]).await?;

            // Their coin leg, if they had already locked it. A position still
            // 'pending' never moved anything, so there is nothing to unwind;
            // one that is 'locked' holds real XLM and has to be released the
            // same way a closed loan releases it — the contract refuses to
            // release a position with no outcome recorded against it, so the
            // outcome goes on chain first. Queue order is by id, which keeps
            // the pair the right way round.
            let locked_position: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM public.xlm_collateral
                  WHERE loan_id = $1 AND status = 'locked'",
            )
            .bind(loan_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| db_err(e, "collateral position"))?;
            if let Some(cid) = locked_position {
                sqlx::query(
                    "INSERT INTO public.collateral_actions (collateral_id, action)
                     VALUES ($1, 'mark_repaid'), ($1, 'release')",
                )
                .bind(cid)
                .execute(&mut *tx)
                .await
                .map_err(|e| db_err(e, "queue release"))?;
            }
            sqlx::query("UPDATE public.loans SET status = 'declined', updated_at = $1 WHERE id = $2")
                .bind(now)
                .bind(loan_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| db_err(e, "decline loan"))?;
            sqlx::query(
                "UPDATE public.loan_guarantors SET status = 'released', updated_at = $1
                  WHERE loan_id = $2 AND status = 'accepted'",
            )
            .bind(now)
            .bind(loan_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_err(e, "release co-guarantors"))?;
            loan_status_out = "declined".to_string();
        }

        commit_event(
            &mut tx,
            EventDraft {
                kind: "guarantee_declined",
                user_id: Some(user_id),
                loan_id: Some(loan_id),
                deposit_id: None,
                rail_ref: None,
                payload: serde_json::json!({ "pledge_amount": pledge_amount }),
                actor_id: Some(user_id),
            },
            &[],
        )
        .await
        .map_err(|e| ledger_err(e, "guarantee_declined"))?;

        tx.commit().await.map_err(|e| db_err(e, "commit respond"))?;
        return Ok(Json(RespondResponse {
            status: "declined",
            loan_status: loan_status_out,
            message: "Invitation declined",
        }));
    }

    // Accept: the pledge freezes NOW, from the guarantor's own withdrawable
    // deposits — vouching with money you don't have isn't a thing.
    lots::freeze_user_lots(&mut tx, user_id, pledge_amount, "pledged", loan_id)
        .await
        .map_err(|_| (
            StatusCode::UNPROCESSABLE_ENTITY,
            "You don't have enough withdrawable deposit to pledge this amount",
        ))?;

    // D1: consent is a recorded round-trip, pinned to the guarantor row.
    let consent_event = commit_event(
        &mut tx,
        EventDraft {
            kind: "guarantee_accepted",
            user_id: Some(user_id),
            loan_id: Some(loan_id),
            deposit_id: None,
            rail_ref: None,
            payload: serde_json::json!({
                "pledge_amount": pledge_amount,
                "consent": "I understand my pledge is frozen and can be seized if the borrower defaults"
            }),
            actor_id: Some(user_id),
        },
        &[],
    )
    .await
    .map_err(|e| ledger_err(e, "guarantee_accepted"))?;

    sqlx::query(
        "UPDATE public.loan_guarantors
            SET status = 'accepted', consent_event = $1, updated_at = $2
          WHERE id = $3",
    )
    .bind(consent_event)
    .bind(now)
    .bind(p.invite_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| db_err(e, "accept invite"))?;

    // The loan funds the moment EVERY leg is in: the accepted pledges covering
    // the guarantors' share, and the borrower's own coins locked on chain if
    // they chose to carry part of their half that way. `try_disburse` owns
    // that decision, so the collateral confirm path and this one can never
    // disagree about what "fully backed" means.
    let accepted: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(pledge_amount), 0)::BIGINT FROM public.loan_guarantors
          WHERE loan_id = $1 AND status = 'accepted'",
    )
    .bind(loan_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| db_err(e, "accepted pledges"))?;

    let funded =
        try_disburse(&mut tx, loan_id, borrower_id, principal, rate_bps, term_months).await?;
    let (loan_status_out, message) = if funded {
        ("active".to_string(), "Pledge locked — the loan is now fully backed and has been disbursed")
    } else if accepted >= guarantor_gap {
        // Pledges are all in; the borrower's own coins are the missing leg.
        ("pending".to_string(), "Pledge locked — waiting on the borrower to lock their XLM")
    } else {
        ("pending".to_string(), "Pledge locked — waiting on the remaining guarantors")
    };

    tx.commit().await.map_err(|e| db_err(e, "commit respond"))?;
    tracing::info!(%user_id, %loan_id, pledge_amount, "guarantee accepted");

    Ok(Json(RespondResponse {
        status: "accepted",
        loan_status: loan_status_out,
        message,
    }))
}
