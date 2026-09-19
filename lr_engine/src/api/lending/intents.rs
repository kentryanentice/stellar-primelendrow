//! Payment intents (041): every PayPal order and Stripe checkout is one the
//! engine asked for, for exactly the amount it decided, and money that arrives
//! is applied only against the intent it was created as.
//!
//! The flow, for both rails and both purposes:
//!
//!   1. **reserve** — under a lock, decide the amount and write the intent
//!      before calling the provider. A deposit is checked against the member's
//!      AML limits here, counting deposits already started; a repayment's
//!      amount is set to exactly what's due, and any older live payment for the
//!      same loan is superseded. No lock is held across the provider call.
//!   2. **open** — record the provider's order/session id on the intent.
//!   3. **claim** — when the member confirms, lock the intent and move it to
//!      `capturing` before capture. This is where a payment for the wrong
//!      member, purpose or loan is refused; for PayPal it happens before any
//!      money moves, so a mismatch costs the member nothing.
//!   4. **consume** — inside the transaction that applies the money, after
//!      re-checking it still matches, mark the intent consumed. The database
//!      trigger from 041 refuses a repayment or deposit ledger event that
//!      doesn't match a claimed intent, so this step can't be skipped.
//!
//! Anything already charged that no longer matches — a Stripe session paid
//! after it was superseded, or a due amount that changed between approval and
//! capture — is refunded through the provider rather than applied.

use axum::http::StatusCode;
use chrono::Utc;
use sqlx::{PgConnection, PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::admin::reconcile;
use super::domain::{self, DepositLimitsFor};
use super::policy;
use super::rails::{self, PaymentRef};
use super::shared::db_err;
use crate::api::users::shared::E;
use crate::infra::{paypal, stripe};

/// How long a started payment stays payable. Stripe's minimum session life is
/// 30 minutes; an hour gives a member time to finish without leaving deposit
/// limits reserved for long.
pub const INTENT_TTL_SECS: i64 = 3600;

/// The intent a provider order/session is about to be created for.
pub struct Reserved {
    pub id: Uuid,
    pub amount: i64,
    pub expires_at: i64,
    /// Older live payments this one replaced, as (rail, provider_ref), so the
    /// caller can close any Stripe page that could still be paid.
    pub superseded: Vec<(String, Option<String>)>,
}

/// A member's deposit headroom right now.
#[derive(serde::Serialize)]
pub struct DepositStatus {
    pub limits: DepositLimitsFor,
    /// Gross deposits in the last 24 hours / 30 days, including deposits
    /// started and not yet finished.
    pub used_24h: i64,
    pub used_30d: i64,
    /// Whole deposit balance, plus deposits started and not yet finished.
    pub balance: i64,
    /// The largest deposit that may be started now.
    pub allowance: i64,
}

fn now() -> i64 {
    Utc::now().timestamp()
}

/// Gross deposits and balance for the AML limits. Credited deposits come from
/// the ledger (so deposits made before 041 count too); deposits started but not
/// finished come from live intents, which is what stops two checkouts opened at
/// the same moment from each fitting under the same limit.
pub async fn deposit_status(
    conn: &mut PgConnection,
    user_id: Uuid,
    params: &policy::PolicyParams,
) -> Result<DepositStatus, E> {
    let score: i16 = sqlx::query_scalar("SELECT score FROM public.credit_scores WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| db_err(e, "deposit limit score"))?
        .unwrap_or(50);
    let limits = domain::deposit_limits_for(score, params);
    let t = now();

    let (credited_24h, credited_30d): (i64, i64) = sqlx::query_as(
        // Gross, from the event: what the member paid in, before any provider
        // fee (043 credits the net, but a limit on money coming in counts all
        // of it). Every deposit event has carried `amount` since the ledger began.
        "SELECT COALESCE(SUM((e.payload->>'amount')::BIGINT) FILTER (WHERE e.created_at >= $2), 0)::BIGINT,
                COALESCE(SUM((e.payload->>'amount')::BIGINT), 0)::BIGINT
           FROM public.ledger_events e
          WHERE e.user_id = $1 AND e.kind = 'deposit_confirmed' AND e.created_at >= $3",
    )
    .bind(user_id)
    .bind(t - 86_400)
    .bind(t - 30 * 86_400)
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| db_err(e, "deposit usage"))?;

    // Live = reserved, capturing, or open and not yet expired.
    let pending: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0)::BIGINT FROM public.payment_intents
          WHERE user_id = $1 AND purpose = 'deposit'
            AND (status IN ('reserved', 'capturing') OR (status = 'open' AND expires_at > $2))",
    )
    .bind(user_id)
    .bind(t)
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| db_err(e, "pending deposits"))?;

    let held: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(amount), 0)::BIGINT FROM public.deposits WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| db_err(e, "deposit balance"))?;

    let (used_24h, used_30d, balance) = (credited_24h + pending, credited_30d + pending, held + pending);
    Ok(DepositStatus {
        limits,
        used_24h,
        used_30d,
        balance,
        allowance: domain::deposit_allowance(limits, used_24h, used_30d, balance),
    })
}

/// Step 1 for a deposit: check the amount against the policy minimum and the
/// member's AML limits, and hold it.
pub async fn reserve_deposit(pool: &PgPool, user_id: Uuid, rail: &str, amount: i64) -> Result<Reserved, E> {
    let rules = policy::active(pool).await?;
    if amount < rules.params.min_deposit {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "That's below the minimum deposit"));
    }

    let mut tx = pool.begin().await.map_err(|e| db_err(e, "begin reserve deposit"))?;
    // One member's deposits are decided one at a time, so two checkouts opened
    // together are counted against the limits together.
    sqlx::query("SELECT id FROM public.users WHERE id = $1 FOR UPDATE")
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| db_err(e, "lock member"))?;

    // Payments they started and never finished stop counting against their
    // limits the moment they time out; this just makes the rows say so.
    expire_stale(&mut tx, user_id).await?;

    let status = deposit_status(&mut tx, user_id, &rules.params).await?;
    if amount > status.allowance {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "That's more than your deposit limit allows right now — check your remaining limit and try a smaller amount",
        ));
    }

    // The fee the provider is expected to keep. The member is credited the
    // net of the fee the provider actually reports (043); this is recorded so
    // the two can be compared.
    let fee = domain::receive_fee_estimate(amount, rules.params.payment_fees.for_rail(rail));
    let expires_at = now() + INTENT_TTL_SECS;
    let id = insert(&mut tx, user_id, "deposit", None, None, amount, fee, None, rail, expires_at).await?;
    tx.commit().await.map_err(|e| db_err(e, "commit reserve deposit"))?;
    Ok(Reserved { id, amount, expires_at, superseded: Vec::new() })
}

/// Step 1 for a repayment: the amount is exactly what's due — the earliest
/// unpaid installment, or a settling loan's arrears — whatever the page asked
/// for. Supersedes any older payment for this loan that hasn't been claimed.
pub async fn reserve_repay(pool: &PgPool, user_id: Uuid, rail: &str, loan_id: Uuid) -> Result<Reserved, E> {
    let rules = policy::active(pool).await?;
    let mut tx = pool.begin().await.map_err(|e| db_err(e, "begin reserve repay"))?;
    let (due, installment) = amount_due(&mut tx, loan_id, user_id).await?;

    let capturing: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM public.payment_intents
          WHERE loan_id = $1 AND purpose = 'repay' AND status = 'capturing')",
    )
    .bind(loan_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| db_err(e, "capturing check"))?;
    if capturing {
        return Err((
            StatusCode::CONFLICT,
            "A payment for this loan is already being processed — wait for it to finish",
        ));
    }

    let superseded: Vec<(String, Option<String>)> = sqlx::query_as(
        "UPDATE public.payment_intents
            SET status = 'superseded', updated_at = $2
          WHERE loan_id = $1 AND purpose = 'repay' AND status IN ('reserved', 'open')
      RETURNING rail, provider_ref",
    )
    .bind(loan_id)
    .bind(now())
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| db_err(e, "supersede old payments"))?;

    // The borrower pays the provider's fee on top (043): the checkout total is
    // the smallest amount that leaves exactly what's due after the estimated
    // fee, so the loan receives the installment to the centavo.
    let (total, fee) = domain::gross_up(due, rules.params.payment_fees.for_rail(rail));
    let expires_at = now() + INTENT_TTL_SECS;
    let id = insert(&mut tx, user_id, "repay", Some(loan_id), installment, total, fee, Some(due), rail, expires_at).await?;
    tx.commit().await.map_err(|e| db_err(e, "commit reserve repay"))?;
    Ok(Reserved { id, amount: total, expires_at, superseded })
}

/// What a loan owes on its next payment, under the loan's row lock: exactly the
/// earliest unpaid installment for a running loan, exactly the arrears for one
/// being settled. Refuses a loan that can't take money or has nothing due.
pub async fn amount_due(
    tx: &mut Transaction<'_, Postgres>,
    loan_id: Uuid,
    user_id: Uuid,
) -> Result<(i64, Option<i16>), E> {
    let loan: Option<(Uuid, String)> =
        sqlx::query_as("SELECT borrower_id, status FROM public.loans WHERE id = $1 FOR UPDATE")
            .bind(loan_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(|e| db_err(e, "lock loan"))?;
    let (borrower_id, status) = loan.ok_or((StatusCode::NOT_FOUND, "Loan not found"))?;
    if borrower_id != user_id {
        return Err((StatusCode::NOT_FOUND, "Loan not found"));
    }
    match status.as_str() {
        "active" => {
            let rows: Vec<(i16, i64, i64, i64, i64)> = sqlx::query_as(
                "SELECT installment, interest_due, interest_paid, principal_due, principal_paid
                   FROM public.loan_schedule WHERE loan_id = $1 ORDER BY installment",
            )
            .bind(loan_id)
            .fetch_all(&mut **tx)
            .await
            .map_err(|e| db_err(e, "schedule due"))?;
            domain::next_installment_due(&rows)
                .map(|(installment, owed)| (owed, Some(installment)))
                .ok_or((StatusCode::CONFLICT, "Nothing is due on this loan"))
        }
        "reconciling" => {
            let owed = reconcile::arrears(tx, loan_id).await?;
            if owed <= 0 {
                return Err((
                    StatusCode::CONFLICT,
                    "There's nothing left to settle on this loan — an administrator just needs to confirm it",
                ));
            }
            Ok((owed, None))
        }
        _ => Err((StatusCode::CONFLICT, "This loan is not accepting payments")),
    }
}

#[allow(clippy::too_many_arguments)]
async fn insert(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    purpose: &str,
    loan_id: Option<Uuid>,
    installment: Option<i16>,
    amount: i64,
    fee: i64,
    applies: Option<i64>,
    rail: &str,
    expires_at: i64,
) -> Result<Uuid, E> {
    sqlx::query_scalar(
        "INSERT INTO public.payment_intents (user_id, purpose, loan_id, installment, amount, fee, applies, rail, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         RETURNING id",
    )
    .bind(user_id)
    .bind(purpose)
    .bind(loan_id)
    .bind(installment)
    .bind(amount)
    .bind(fee)
    .bind(applies)
    .bind(rail)
    .bind(expires_at)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| {
        if e.as_database_error().is_some_and(|d| d.is_unique_violation()) {
            // Two payments for one loan started at the same instant; the other
            // one won the index.
            return (StatusCode::CONFLICT, "A payment for this loan was just started — try again");
        }
        db_err(e, "insert payment intent")
    })
}

/// Step 2: the provider's order/session exists. Returns false when the intent
/// was superseded while the provider was being called, so the caller can close
/// the page it just created instead of handing it out.
pub async fn open(pool: &PgPool, id: Uuid, provider_ref: &str) -> Result<bool, E> {
    let updated = sqlx::query(
        "UPDATE public.payment_intents
            SET provider_ref = $2, status = 'open', updated_at = $3
          WHERE id = $1 AND status = 'reserved'",
    )
    .bind(id)
    .bind(provider_ref)
    .bind(now())
    .execute(pool)
    .await
    .map_err(|e| db_err(e, "open payment intent"))?;
    Ok(updated.rows_affected() == 1)
}

/// The provider call in step 2 failed: release what was reserved.
pub async fn abandon(pool: &PgPool, id: Uuid) {
    let result = sqlx::query(
        "UPDATE public.payment_intents SET status = 'expired', updated_at = $2
          WHERE id = $1 AND status = 'reserved'",
    )
    .bind(id)
    .bind(now())
    .execute(pool)
    .await;
    if let Err(e) = result {
        tracing::error!(%id, "could not release payment intent: {e}");
    }
}

/// The member came back from Stripe without paying: close the checkout page so
/// it can never be paid, then release the payment.
///
/// The order matters. A Stripe page stays payable after the member navigates
/// away, so the record is only released once Stripe confirms the page is shut.
/// If Stripe refuses — which is what happens when the page was in fact
/// completed — the record is left exactly as it was, so the payment can still
/// be applied (or refunded) through the ordinary path. Nothing is charged and
/// nothing is credited here either way.
pub async fn cancel_stripe(pool: &PgPool, user_id: Uuid, purpose: &str) -> Result<(), E> {
    // The member's own live Stripe pages for this purpose. The cancel redirect
    // carries no session id — Stripe only substitutes one into the success
    // URL — so they are found by owner, which is equally specific: a member
    // has at most one live checkout per purpose.
    let sessions: Vec<(Uuid, Option<String>)> = sqlx::query_as(
        "SELECT id, provider_ref FROM public.payment_intents
          WHERE user_id = $1 AND purpose = $2 AND rail = 'stripe'
            AND status = 'open' AND expires_at > $3",
    )
    .bind(user_id)
    .bind(purpose)
    .bind(now())
    .fetch_all(pool)
    .await
    .map_err(|e| db_err(e, "live stripe payments"))?;

    for (id, provider_ref) in sessions {
        let Some(session_id) = provider_ref else { continue };
        if stripe::expire_session(&session_id).await.is_err() {
            // Already completed, already expired, or Stripe was unreachable.
            // Leaving it open is the safe reading: a paid page must still be
            // applicable, and an unpaid one expires on its own within the hour.
            tracing::info!(%user_id, session_id, "stripe page not closed on cancel — left to expire");
            continue;
        }
        let released = sqlx::query(
            "UPDATE public.payment_intents
                SET status = 'expired', updated_at = $2, last_error = 'Cancelled by the member'
              WHERE id = $1 AND status = 'open'",
        )
        .bind(id)
        .bind(now())
        .execute(pool)
        .await
        .map_err(|e| db_err(e, "release stripe payment"))?;
        if released.rows_affected() == 1 {
            tracing::info!(%user_id, session_id, "stripe checkout cancelled by the member");
        }
    }
    Ok(())
}

/// The member closed PayPal's window without approving: release the payment
/// now rather than leaving it holding their deposit limit until it expires.
///
/// PayPal only, and deliberately: a PayPal order cannot be captured without a
/// claim, so releasing one is safe. Stripe's equivalent is `cancel_stripe`,
/// which has to close the page with Stripe first.
///
/// Nothing happens if it was already approved, captured or replaced.
pub async fn cancel(pool: &PgPool, user_id: Uuid, provider_ref: &str) -> Result<(), E> {
    sqlx::query(
        "UPDATE public.payment_intents
            SET status = 'expired', updated_at = $3, last_error = 'Cancelled by the member'
          WHERE provider_ref = $1 AND user_id = $2 AND rail = 'paypal' AND status = 'open'",
    )
    .bind(provider_ref)
    .bind(user_id)
    .bind(now())
    .execute(pool)
    .await
    .map_err(|e| db_err(e, "cancel payment intent"))?;
    Ok(())
}

/// Marks a member's timed-out payments expired. Nothing depends on it — every
/// query that counts live payments already tests `expires_at` — but it keeps
/// abandoned rows from sitting in `open` forever, which reads as if the member
/// still has a payment waiting.
async fn expire_stale(tx: &mut Transaction<'_, Postgres>, user_id: Uuid) -> Result<(), E> {
    sqlx::query(
        "UPDATE public.payment_intents
            SET status = 'expired', updated_at = $2
          WHERE user_id = $1 AND status IN ('reserved', 'open') AND expires_at <= $2",
    )
    .bind(user_id)
    .bind(now())
    .execute(&mut **tx)
    .await
    .map_err(|e| db_err(e, "expire stale payments"))?;
    Ok(())
}

/// Closes Stripe pages a newer payment replaced, so they can't be paid. Best
/// effort — one paid in the gap is refunded when it's confirmed.
pub async fn close_superseded(superseded: &[(String, Option<String>)]) {
    for (rail, provider_ref) in superseded {
        if let (true, Some(session_id)) = (rail == "stripe", provider_ref.as_deref()) {
            let _ = stripe::expire_session(session_id).await;
        }
    }
}

/// How long a payment may sit mid-capture before the sweep below treats it as
/// interrupted. Comfortably longer than a capture takes (a second or two) and
/// than the provider call's own timeout, so a payment in flight is never
/// touched.
const STUCK_CAPTURE_SECS: i64 = 10 * 60;
/// How often the sweep looks. The first pass runs at startup, which is when
/// an engine that died mid-payment comes back.
const CAPTURE_SWEEP_SECS: u64 = 5 * 60;

/// Recovers payments the engine was interrupted in the middle of capturing.
///
/// Claiming an intent marks it `capturing`, and every path that claims one
/// also releases it — unless the engine itself stops answering: a restart
/// mid-payment, or the database going away between the provider's "yes" and
/// the ledger write. The row is then stuck: it blocks the loan's next payment
/// (one live repayment per loan), and if the provider *did* take the money,
/// that money is in the platform's balance and in nobody's books.
///
/// So, for each stuck payment, ask the provider what actually happened:
/// - **Money was taken** — finish the payment: apply the repayment to the loan,
///   or credit the deposit, exactly as the interrupted request would have. The
///   member paid what was due and they get what they paid for; being charged
///   and then refunded, only to pay again, is not a resolution. The apply path
///   is the same code the request uses, so it is just as exact — and if what
///   arrived no longer matches what the loan owes, *that* path refunds it.
/// - **Nothing was taken** — mark it expired, which releases the loan.
/// - **The provider can't be reached** — leave it for the next sweep. A
///   payment is never concluded on a guess.
pub fn spawn_capture_recovery(pool: PgPool) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(CAPTURE_SWEEP_SECS));
        loop {
            ticker.tick().await;
            recover_stuck_captures(&pool).await;
        }
    });
}

/// id, user_id, purpose, rail, provider_ref, loan_id, amount, applies
type StuckRow = (Uuid, Uuid, String, String, String, Option<Uuid>, i64, Option<i64>);

async fn recover_stuck_captures(pool: &PgPool) {
    let cutoff = now() - STUCK_CAPTURE_SECS;
    let stuck: Vec<StuckRow> = match sqlx::query_as(
        "SELECT id, user_id, purpose, rail, provider_ref, loan_id, amount, applies
           FROM public.payment_intents
          WHERE status = 'capturing' AND provider_ref IS NOT NULL AND updated_at <= $1
          ORDER BY updated_at",
    )
    .bind(cutoff)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!("stuck capture sweep: {e}");
            return;
        }
    };

    for (id, user_id, purpose, rail, provider_ref, loan_id, amount, applies) in stuck {
        let owner = user_id.to_string();
        let found = match rail.as_str() {
            "paypal" => paypal::captured_order(&provider_ref, &owner).await,
            // A Stripe session is read, never captured, so asking is safe: a
            // paid one comes back, an unpaid one is "not completed".
            "stripe" => match stripe::capture_session(&provider_ref, &owner).await {
                Ok(captured) => Ok(Some(captured)),
                Err("Payment was not completed") => Ok(None),
                Err(e) => Err(e),
            },
            other => {
                tracing::error!(intent = %id, rail = other, "stuck capture on an unknown rail");
                continue;
            }
        };

        match found {
            Ok(Some(captured)) => {
                let rail: &'static str = if rail == "paypal" { "paypal" } else { "stripe" };
                let claimed = Claimed { id, rail, loan_id, amount, applies };
                tracing::warn!(intent = %id, %provider_ref, %purpose, "interrupted payment was charged — finishing it");
                // The intent is still `capturing`, which is exactly the state
                // both paths expect to consume, so this is the interrupted
                // request resuming rather than a second one.
                let finished = match (purpose.as_str(), loan_id) {
                    ("repay", Some(loan)) => {
                        let rules = match policy::active(pool).await {
                            Ok(rules) => rules,
                            Err(_) => continue,
                        };
                        super::repay::apply_captured(pool, &rules, user_id, loan, &claimed, rail, &captured)
                            .await
                            .map(|applied| format!(
                                "applied {} to loan {loan} ({} interest, {} principal)",
                                applied.amount_received, applied.interest_paid, applied.principal_paid
                            ))
                    }
                    ("deposit", _) => {
                        super::deposit::credit(pool, user_id, &claimed, rails::Captured { rail, payment: captured })
                            .await
                            .map(|credited| format!("credited {} to the member's balance", credited.amount))
                    }
                    (other, _) => {
                        tracing::error!(intent = %id, purpose = other, "stuck capture with no way to finish it");
                        continue;
                    }
                };
                match finished {
                    Ok(what) => tracing::warn!(intent = %id, %provider_ref, "interrupted payment finished — {what}"),
                    // `apply_captured` and `credit` refund anything they can't
                    // apply, so the money is settled either way; this is the
                    // outcome, not an unresolved error.
                    Err((status, message)) => {
                        tracing::warn!(intent = %id, %provider_ref, %status, "interrupted payment not applied — {message}")
                    }
                }
            }
            Ok(None) => {
                let released = sqlx::query(
                    "UPDATE public.payment_intents
                        SET status = 'expired', last_error = $2, updated_at = $3
                      WHERE id = $1 AND status = 'capturing'",
                )
                .bind(id)
                .bind("the engine was interrupted mid-payment; nothing was charged")
                .bind(now())
                .execute(pool)
                .await;
                match released {
                    Ok(_) => tracing::warn!(intent = %id, %provider_ref, "interrupted payment was never charged — released"),
                    Err(e) => tracing::error!(intent = %id, "could not release interrupted payment: {e}"),
                }
            }
            // Say nothing about the money we couldn't ask about; try again.
            Err(e) => tracing::error!(intent = %id, %provider_ref, "stuck capture not resolved: {e}"),
        }
    }
}

/// An intent claimed for capture.
pub struct Claimed {
    pub id: Uuid,
    pub rail: &'static str,
    pub loan_id: Option<Uuid>,
    /// What the member is charged, gross.
    pub amount: i64,
    /// Repayments: exactly what the payment applies to the loan (`amount`
    /// less the fee charged on top). `None` on a deposit, and on a repayment
    /// started before 043, which applies all of `amount`.
    pub applies: Option<i64>,
}

/// id, user_id, purpose, loan_id, amount, applies, rail, status, expires_at
type IntentRow = (Uuid, Uuid, String, Option<Uuid>, i64, Option<i64>, String, String, i64);

/// What claiming found.
pub enum Claim {
    /// Capture it and apply it.
    Proceed(Claimed),
    /// A Stripe session whose intent was superseded or expired. It may still
    /// have been paid; the caller reads it and refunds it if so. (A PayPal
    /// order in the same state is refused outright — it can't have been
    /// captured, because capture only ever happens after a claim.)
    Stale(Claimed),
}

/// Step 3: lock the intent the member is presenting and move it to capturing.
pub async fn claim(pool: &PgPool, user_id: Uuid, payment: &PaymentRef, purpose: &str) -> Result<Claim, E> {
    let (rail, provider_ref) = match (
        payment.order_id.as_deref().map(str::trim).filter(|s| !s.is_empty()),
        payment.session_id.as_deref().map(str::trim).filter(|s| !s.is_empty()),
    ) {
        (Some(_), Some(_)) => {
            return Err((StatusCode::UNPROCESSABLE_ENTITY, "Send either a PayPal order or a Stripe session, not both"));
        }
        (Some(order_id), None) => ("paypal", order_id.to_string()),
        (None, Some(session_id)) => ("stripe", session_id.to_string()),
        (None, None) => return Err((StatusCode::UNPROCESSABLE_ENTITY, "No payment reference — nothing to confirm")),
    };

    let mut tx = pool.begin().await.map_err(|e| db_err(e, "begin claim"))?;
    let row: Option<IntentRow> = sqlx::query_as(
        "SELECT id, user_id, purpose, loan_id, amount, applies, rail, status, expires_at
           FROM public.payment_intents WHERE provider_ref = $1 FOR UPDATE",
    )
    .bind(&provider_ref)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| db_err(e, "claim payment intent"))?;
    let (id, owner, intent_purpose, loan_id, amount, applies, intent_rail, status, expires_at) = row.ok_or((
        StatusCode::UNPROCESSABLE_ENTITY,
        "That payment wasn't started here — please start a new one",
    ))?;

    if owner != user_id {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "That payment doesn't belong to this account"));
    }
    if intent_purpose != purpose || intent_rail != rail {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "That payment was started for something else"));
    }
    let claimed = Claimed { id, rail, loan_id, amount, applies };

    match status.as_str() {
        "consumed" => Err((StatusCode::CONFLICT, "This payment was already processed")),
        "refunded" | "refund_failed" => Err((StatusCode::CONFLICT, "This payment was refunded")),
        "reserved" => Err((StatusCode::CONFLICT, "This payment isn't ready yet")),
        "superseded" | "expired" if rail == "stripe" => Ok(Claim::Stale(claimed)),
        "superseded" | "expired" => Err((
            StatusCode::CONFLICT,
            "This payment was replaced by a newer one or expired — nothing was charged. Please start again",
        )),
        "open" if rail == "paypal" && expires_at < now() => {
            sqlx::query("UPDATE public.payment_intents SET status = 'expired', updated_at = $2 WHERE id = $1")
                .bind(id)
                .bind(now())
                .execute(&mut *tx)
                .await
                .map_err(|e| db_err(e, "expire payment intent"))?;
            tx.commit().await.map_err(|e| db_err(e, "commit expire"))?;
            Err((StatusCode::CONFLICT, "This payment expired — nothing was charged. Please start again"))
        }
        // "open", or "capturing" on a retried confirmation.
        _ => {
            sqlx::query("UPDATE public.payment_intents SET status = 'capturing', updated_at = $2 WHERE id = $1")
                .bind(id)
                .bind(now())
                .execute(&mut *tx)
                .await
                .map_err(|e| db_err(e, "claim for capture"))?;
            tx.commit().await.map_err(|e| db_err(e, "commit claim"))?;
            Ok(Claim::Proceed(claimed))
        }
    }
}

/// Capture didn't happen (the provider said unpaid, or couldn't be reached):
/// put the intent back so the member can confirm again. A PayPal capture that
/// did go through but whose answer was lost is safe to retry — PayPal returns
/// the same capture, and the ledger's unique rail_ref blocks a double credit.
pub async fn release_claim(pool: &PgPool, id: Uuid) {
    let result = sqlx::query(
        "UPDATE public.payment_intents SET status = 'open', updated_at = $2
          WHERE id = $1 AND status = 'capturing'",
    )
    .bind(id)
    .bind(now())
    .execute(pool)
    .await;
    if let Err(e) = result {
        tracing::error!(%id, "could not release payment claim: {e}");
    }
}

/// Step 4, inside the transaction that applies the money: record the capture
/// and mark the intent consumed. Fails if the intent is no longer claimed —
/// nothing else may have touched it between claim and apply.
pub async fn consume(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    capture_ref: &str,
    provider_fee: i64,
) -> Result<(), E> {
    let updated = sqlx::query(
        "UPDATE public.payment_intents
            SET status = 'consumed', capture_ref = $2, provider_fee = $4, updated_at = $3
          WHERE id = $1 AND status = 'capturing'",
    )
    .bind(id)
    .bind(capture_ref)
    .bind(now())
    .bind(provider_fee)
    .execute(&mut **tx)
    .await
    .map_err(|e| db_err(e, "consume payment intent"))?;
    if updated.rows_affected() != 1 {
        return Err((StatusCode::CONFLICT, "This payment was already processed"));
    }
    Ok(())
}

/// Gives back money that arrived but can't be applied, and records that it
/// did. Returns the message to show the member.
pub async fn refund(pool: &PgPool, claimed: &Claimed, capture_ref: &str, why: &'static str) -> E {
    let key = claimed.id.to_string();
    let result = match claimed.rail {
        "paypal" => paypal::refund_capture(capture_ref, &key).await,
        _ => stripe::refund_payment(capture_ref, &key).await,
    };
    let (status, error) = match result {
        Ok(()) => ("refunded", None),
        Err(e) => ("refund_failed", Some(e)),
    };
    let recorded = sqlx::query(
        "UPDATE public.payment_intents
            SET status = $2, capture_ref = $3, last_error = $4, updated_at = $5
          WHERE id = $1",
    )
    .bind(claimed.id)
    .bind(status)
    .bind(capture_ref)
    .bind(error.map(|e| format!("{why} — refund: {e}")).unwrap_or_else(|| why.to_string()))
    .bind(now())
    .execute(pool)
    .await;
    if let Err(e) = recorded {
        tracing::error!(intent = %claimed.id, capture_ref, "refund outcome not recorded: {e}");
    }
    match error {
        None => {
            tracing::warn!(intent = %claimed.id, capture_ref, why, "payment refunded");
            (StatusCode::CONFLICT, "This payment no longer matched what was due, so it was refunded in full")
        }
        Some(e) => {
            tracing::error!(intent = %claimed.id, capture_ref, why, e, "REFUND FAILED — operator action needed");
            (
                StatusCode::CONFLICT,
                "This payment no longer matched what was due and couldn't be refunded automatically — support has been alerted",
            )
        }
    }
}

/// A Stripe session whose intent was superseded or expired: if it was paid
/// anyway (in the moment before its page was closed), refund it. Returns the
/// message for the member either way.
pub async fn refund_stale(pool: &PgPool, user_id: Uuid, payment: &PaymentRef, claimed: &Claimed) -> E {
    match rails::capture(payment, user_id).await {
        Ok(captured) => refund(pool, claimed, &captured.payment.capture_id, "paid after it was replaced or expired").await,
        Err(_) => (
            StatusCode::CONFLICT,
            "This payment was replaced by a newer one or expired — nothing was charged. Please start again",
        ),
    }
}

/// Whether the intent behind a provider reference is one that can no longer be
/// applied (superseded or expired). The Stripe webhook uses it to spot a paid
/// repayment page that needs refunding without claiming a live one.
pub async fn is_stale(pool: &PgPool, provider_ref: &str) -> Result<bool, E> {
    let status: Option<String> = sqlx::query_scalar("SELECT status FROM public.payment_intents WHERE provider_ref = $1")
        .bind(provider_ref)
        .fetch_optional(pool)
        .await
        .map_err(|e| db_err(e, "intent status"))?;
    Ok(matches!(status.as_deref(), Some("superseded" | "expired")))
}
