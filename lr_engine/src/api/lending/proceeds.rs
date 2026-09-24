//! Credits proceeds that loans disbursed before 050 never paid out.
//!
//! Before 050, disbursement booked a loan's proceeds as owed out
//! (`payout_payable`) and nothing sent them until the borrower pressed "send to
//! PayPal" — which only worked while the loan was active. Every loan repaid
//! before anyone pressed it left its proceeds owed with no way to claim them.
//! Disbursement now credits the borrower's pool balance directly
//! (`shared::disburse`); this pass gives the older loans the same treatment:
//! whatever they still owe is moved into the borrower's balance as a
//! withdrawable lot, exactly as a new disbursement would have put it there.
//!
//! Safe to run any number of times. Each credit carries the ledger reference
//! `loan_proceeds:<loan>`, which the ledger's unique index accepts once, so a
//! second pass — or two engines running it at once — credits nothing twice.
//! A loan whose proceeds were paid out (or are on their way) is left alone; a
//! failed payout means the money never left, so it is credited like any other.
//!
//! Runs at boot and then hourly. The hourly pass is only a backstop — after the
//! first one there is nothing left for it to find, since no loan is disbursed
//! the old way any more.

use sqlx::PgPool;
use uuid::Uuid;
use std::time::Duration;

use super::ledger::{EventDraft, LedgerError, Posting, commit_event};

const SWEEP_INTERVAL: Duration = Duration::from_secs(60 * 60);

pub fn spawn_legacy_proceeds(pool: PgPool) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(SWEEP_INTERVAL);
        loop {
            ticker.tick().await;
            if let Err(e) = credit_all(&pool).await {
                tracing::error!("legacy proceeds pass failed: {e}");
            }
        }
    });
}

async fn credit_all(pool: &PgPool) -> Result<(), sqlx::Error> {
    // Disbursed the old way — the disbursement event owes the proceeds out —
    // and nothing sent, sending, or paid against it since.
    let owed: Vec<(Uuid, Uuid, i64)> = sqlx::query_as(
        "SELECT l.id, l.borrower_id, l.principal
           FROM public.loans l
          WHERE l.disbursed_at IS NOT NULL
            AND EXISTS (SELECT 1 FROM public.ledger_postings p
                          JOIN public.ledger_events e ON e.id = p.event_id
                         WHERE e.loan_id = l.id AND e.kind = 'loan_disbursed'
                           AND p.account = 'payout_payable')
            AND NOT EXISTS (SELECT 1 FROM public.payouts po
                             WHERE po.loan_id = l.id AND po.status <> 'failed')
            AND NOT EXISTS (SELECT 1 FROM public.ledger_events c
                             WHERE c.rail_ref = 'loan_proceeds:' || l.id::text)
          ORDER BY l.disbursed_at",
    )
    .fetch_all(pool)
    .await?;

    for (loan_id, borrower_id, principal) in owed {
        // One transaction per loan: independent credits to different people,
        // and one bad row should not hold back everyone else's.
        if let Err(e) = credit_one(pool, loan_id, borrower_id, principal).await {
            tracing::error!(%loan_id, "legacy proceeds credit failed: {e}");
        }
    }
    Ok(())
}

async fn credit_one(pool: &PgPool, loan_id: Uuid, borrower_id: Uuid, principal: i64) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    // The loan row serializes this against anything else touching the loan,
    // and the payout check runs again under it: a payout that appeared since
    // the scan means the money is on its way, and crediting it too would pay
    // it twice.
    sqlx::query("SELECT id FROM public.loans WHERE id = $1 FOR UPDATE")
        .bind(loan_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    let paid: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM public.payouts WHERE loan_id = $1 AND status <> 'failed')",
    )
    .bind(loan_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    if paid {
        return Ok(());
    }

    let lot_id: Uuid = sqlx::query_scalar(
        "INSERT INTO public.deposits (user_id, amount, badge, origin)
         VALUES ($1, $2, 'available', 'loan_proceeds') RETURNING id",
    )
    .bind(borrower_id)
    .bind(principal)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    let posted = commit_event(
        &mut tx,
        EventDraft {
            kind: "loan_proceeds_credited",
            user_id: Some(borrower_id),
            loan_id: Some(loan_id),
            deposit_id: Some(lot_id),
            rail_ref: Some(format!("loan_proceeds:{loan_id}")),
            payload: serde_json::json!({ "amount": principal, "reason": "never paid out" }),
            actor_id: None,
        },
        // The promise to pay out is cancelled and becomes a deposit — the same
        // two accounts a refunded withdrawal moves, the other way round.
        &[
            Posting { account: "payout_payable", amount: principal },
            Posting { account: "member_deposits", amount: -principal },
        ],
    )
    .await;
    match posted {
        Ok(_) => {}
        // Already credited by another pass: nothing to do, and the lot above
        // goes with the rollback.
        Err(LedgerError::DuplicateRail) => return Ok(()),
        Err(LedgerError::Db(e)) => return Err(e.to_string()),
        Err(LedgerError::Unbalanced(net)) => return Err(format!("unbalanced by {net}")),
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    tracing::info!(%loan_id, %borrower_id, principal, "unpaid loan proceeds credited to balance");
    Ok(())
}
