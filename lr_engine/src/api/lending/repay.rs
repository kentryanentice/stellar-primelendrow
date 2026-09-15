//! POST /loans/repay — PHP money-in against an active loan.
//!
//! PayPal is captured server-side first (never inside the transaction); the
//! captured centavos are then allocated oldest installment first — each
//! installment's interest then its principal — so a payment clears whole
//! installments in order (Lesson 8), the interest is split four ways
//! (platform, reserve, every depositor pro-rata to their balance, and the
//! risk band: guarantors by their own tier, the rest to the recovery fund),
//! 'lent' funding lots unlock as principal returns,
//! and a fully paid loan releases its collateral/pledges and bumps the
//! borrower's score. A duplicate capture bounces off the ledger's rail_ref.

use axum::{Extension, Json, http::{HeaderMap, StatusCode}};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::domain;
use super::intents::{self, Claim};
use super::ledger::{EventDraft, Posting, commit_event};
use super::lots;
use super::policy;
use super::rails::{self, Captured, PaymentRef};
// Settling a reopened default is an admin-initiated flow, but the payment
// itself arrives here on the borrower's own rail — so this is the one place
// the borrower side reaches into `admin`, and only for the split and arrears.
use super::admin::reconcile;
use super::shared::{db_err, ledger_err};
use crate::api::users::shared::{E, require_verified_user};

/// Score movement on a fully repaid loan. Kept here (not policy JSON) until
/// scoring gets its own policy slice — it's one number, and the log records
/// every application of it.
const SCORE_BUMP_ON_CLOSE: i16 = 5;

#[derive(Deserialize)]
pub struct RepayInput {
    loan_id: Uuid,
    /// Flattened, so the PayPal client's `{"loan_id": …, "order_id": …}` still
    /// parses unchanged and `session_id` is the Stripe equivalent.
    #[serde(flatten)]
    payment: PaymentRef,
}

#[derive(Serialize)]
pub struct RepayResponse {
    /// What was applied to the loan.
    pub amount_received: i64,
    /// The payment-provider fee paid on top of it (043).
    pub fee_paid: i64,
    pub interest_paid: i64,
    pub principal_paid: i64,
    /// Anything beyond what the loan owed becomes a fresh deposit lot.
    pub excess_to_deposit: i64,
    pub principal_outstanding: i64,
    pub loan_status: &'static str,
    pub message: &'static str,
}

/// One member's slice of a repayment's interest, as recorded in
/// `member_interest` (039).
struct MemberCredit {
    user_id: Uuid,
    role: &'static str,
    /// Deposit balance for a depositor, pledge for a guarantor.
    weight: i64,
    /// The tier percent a guarantor was paid at.
    tier_share: Option<i16>,
    amount: i64,
}

struct ScheduleRow {
    id: i64,
    installment: i16,
    interest_due: i64,
    interest_paid: i64,
    principal_due: i64,
    principal_paid: i64,
}

pub async fn repay(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(p): Json<RepayInput>,
) -> Result<Json<RepayResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    let rules = policy::active(&pool).await?;

    // The payment must be a repayment the engine started, for this member and
    // THIS loan, for exactly what was due when it started (041). Claimed before
    // capture: a PayPal order that doesn't qualify is refused without being
    // charged, and no newer payment can replace it mid-capture.
    let claimed = match intents::claim(&pool, user_id, &p.payment, "repay").await? {
        Claim::Proceed(claimed) => claimed,
        Claim::Stale(claimed) => return Err(intents::refund_stale(&pool, user_id, &p.payment, &claimed).await),
    };
    if claimed.loan_id != Some(p.loan_id) {
        intents::release_claim(&pool, claimed.id).await;
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "That payment was started for a different loan"));
    }

    // Verify before the transaction: no locks held across a payment provider.
    let Captured { rail, payment: captured } = match rails::capture(&p.payment, user_id).await {
        Ok(captured) => captured,
        Err(e) => {
            intents::release_claim(&pool, claimed.id).await;
            return Err(e);
        }
    };
    // Three numbers, kept apart (043):
    //   received      the gross the borrower paid
    //   applies       exactly what reaches the loan — the installment or arrears
    //   provider_fee  what PayPal/Stripe actually kept, as they report it
    // The borrower paid `received - applies` on top as the estimated fee.
    let received = captured.centavos;
    let applies = claimed.applies.unwrap_or(claimed.amount);
    let rules_fees = rules.params.payment_fees.for_rail(rail);
    let provider_fee = captured.fee.unwrap_or_else(|| domain::receive_fee_estimate(received, rules_fees));
    let fee_paid = received - applies;
    // Estimate minus reality: positive if the provider kept less than the
    // borrower paid for, negative if more. Booked so `cash` matches the
    // provider balance exactly.
    let fee_variance = fee_paid - provider_fee;

    let mut tx = pool.begin().await.map_err(|e| db_err(e, "begin repay"))?;

    let loan: Option<(Uuid, String, i64, String)> = sqlx::query_as(
        "SELECT borrower_id, product, principal_outstanding, status
           FROM public.loans WHERE id = $1 FOR UPDATE",
    )
    .bind(p.loan_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| db_err(e, "lock loan"))?;
    let (borrower_id, product, outstanding_before, status) =
        loan.ok_or((StatusCode::NOT_FOUND, "Loan not found"))?;
    if borrower_id != user_id {
        return Err((StatusCode::NOT_FOUND, "Loan not found"));
    }
    // `reconciling` is a defaulted loan an admin reopened for settlement (033).
    // It takes payments through this same endpoint on purpose — the borrower
    // pays the way they always did, and the money is verified by the provider
    // like every other peso — but where it lands in the books is different, and
    // that difference is handled at the posting step below.
    let settling = status == "reconciling";
    if status != "active" && !settling {
        return Err((StatusCode::CONFLICT, "This loan is not active"));
    }
    // The arrears (or next installment) are re-checked below, once the rows
    // this payment touches are locked: a payment that no longer matches is
    // refunded, never applied as more or less than what's due.

    // A settlement does not touch the schedule at all — see the block comment
    // on the allocation below for why — so its rows are neither read nor
    // locked. `schedule` stays empty, which makes every loop and check below a
    // no-op without needing a branch of its own.
    let mut schedule: Vec<ScheduleRow> = Vec::new();
    if !settling {
        let rows: Vec<(i64, i16, i64, i64, i64, i64)> = sqlx::query_as(
            "SELECT id, installment, interest_due, interest_paid, principal_due, principal_paid
               FROM public.loan_schedule
              WHERE loan_id = $1
              ORDER BY installment
              FOR UPDATE",
        )
        .bind(p.loan_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| db_err(e, "lock schedule"))?;
        schedule = rows
            .into_iter()
            .map(|(id, installment, interest_due, interest_paid, principal_due, principal_paid)| ScheduleRow {
                id, installment, interest_due, interest_paid, principal_due, principal_paid,
            })
            .collect();
    }

    // Exact, re-checked under the locks: what arrived must be what the intent
    // asked for, and that must still be exactly what's due now. A mismatch here
    // is money already taken, so it is given back in full rather than applied
    // as more or less than the installment (041).
    let due_now = if settling {
        Some(reconcile::arrears(&mut tx, p.loan_id).await?)
    } else {
        let rows: Vec<(i16, i64, i64, i64, i64)> = schedule
            .iter()
            .map(|r| (r.installment, r.interest_due, r.interest_paid, r.principal_due, r.principal_paid))
            .collect();
        domain::next_installment_due(&rows).map(|(_, owed)| owed)
    };
    if received != claimed.amount || due_now != Some(applies) {
        drop(tx);
        return Err(intents::refund(&pool, &claimed, &captured.capture_id, "repayment no longer matched the amount due").await);
    }

    // Allocation, oldest installment first — for an ordinary repayment.
    //
    // A **settlement** skips this entirely, and that is the whole of the fix
    // for "borrowers shouldn't have to pay back all the months". Allocating a
    // settlement across the schedule charged for every unpaid installment,
    // including months that were never due and interest nobody had earned, and
    // it credited none of what the recovery waterfall had already taken from
    // the borrower's own deposits. What a settlement owes is measured in
    // `reconcile::arrears` instead — the money other parties are still out of
    // pocket — and the schedule is left as the default stamped it, because
    // those installments really were defaulted.
    //
    // Allocation, oldest installment first: within each installment settle its
    // interest THEN its principal before moving to the next. A payment clears
    // whole installments in order — overdue interest + principal, then the
    // current one, then future ones — instead of vacuuming every month's
    // interest across the whole loan first (which pre-paid interest that wasn't
    // due and left each "paid" installment still owing principal). Interest is
    // still paid before principal within an installment (Lesson 8). Since 041
    // the payment is exactly the next installment, so nothing is left over:
    // `excess` stays 0, and the database refuses a payment row where it isn't.
    let mut remaining = applies;
    let mut interest_total: i64 = 0;
    let mut principal_total: i64 = 0;
    for row in &mut schedule {
        if remaining == 0 {
            break;
        }
        let interest_owed = row.interest_due - row.interest_paid;
        if interest_owed > 0 {
            let pay = interest_owed.min(remaining);
            row.interest_paid += pay;
            interest_total += pay;
            remaining -= pay;
        }
        if remaining == 0 {
            break;
        }
        let principal_owed = row.principal_due - row.principal_paid;
        if principal_owed > 0 {
            let pay = principal_owed.min(remaining);
            row.principal_paid += pay;
            principal_total += pay;
            remaining -= pay;
        }
    }
    let excess = remaining;

    let now = Utc::now().timestamp();
    for row in &schedule {
        let settled = row.interest_paid >= row.interest_due && row.principal_paid >= row.principal_due;
        sqlx::query(
            "UPDATE public.loan_schedule
                SET interest_paid = $1, principal_paid = $2,
                    status = CASE WHEN $3 THEN 'paid' ELSE status END
              WHERE id = $4",
        )
        .bind(row.interest_paid)
        .bind(row.principal_paid)
        .bind(settled)
        .bind(row.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| db_err(e, "update schedule"))?;
    }

    // The books: one event, postings that tie to the received centavo.
    //
    // `cash` rises by what really reached the provider balance: the gross less
    // the provider's own fee. The loan side is credited exactly `applies`, and
    // the difference between the fee the borrower paid and the fee the
    // provider kept is the variance (043).
    //
    // Two shapes, because the money means two different things.
    let mut postings = vec![Posting { account: "cash", amount: received - provider_fee }];
    if fee_variance != 0 {
        postings.push(Posting { account: "payment_fee_variance", amount: -fee_variance });
    }
    // The interest split exactly as booked below, recorded beside the event
    // (037), and every member's slice of it (039). All stay empty on a
    // settlement, which collects no interest.
    // With the two totals its member slices were divided by (040).
    let mut booked_split: Option<(domain::InterestParts, i64, i64)> = None;
    let mut member_credits: Vec<MemberCredit> = Vec::new();
    let settlement = if settling {
        // Settling a default. `loans_receivable` for this loan is already zero
        // — `recovery::advance` wrote the whole debt off when it took the
        // borrower's deposits, charged the guarantors and booked the remainder
        // as a loss. Crediting a receivable that no longer exists would drive
        // the pool's assets negative, and recognising interest income on a loan
        // the pool has already written off would book a profit twice.
        //
        // So the payment undoes the loss instead: guarantors made whole first,
        // then the reserve, then anything left back to the borrower. See
        // `reconcile::settle` for why that order and not the strict reverse.
        let split = reconcile::settle(&mut tx, p.loan_id, user_id, applies).await?;
        if split.to_guarantors > 0 {
            postings.push(Posting { account: "member_deposits", amount: -split.to_guarantors });
        }
        if split.to_reserve > 0 {
            postings.push(Posting { account: "reserve_fund", amount: -split.to_reserve });
        }   
        if split.to_borrower > 0 {
            postings.push(Posting { account: "member_deposits", amount: -split.to_borrower });
        }
        Some(split)
    } else {
        // Who shares the interest, read before this payment credits or unlocks
        // anything, so the split reflects the pool as it stood when the money
        // arrived.
        //
        //   * Depositors: every member, by whole deposit balance.
        //   * Guarantors: the loan's accepted guarantors, each at the tier of
        //     their OWN score, weighted by pledge.
        //
        // `book_interest` guarantees every slice sums back to its part.
        let balances = lots::deposit_balances(&mut tx).await?;
        let guarantors: Vec<(Uuid, i64, i16)> = sqlx::query_as(
            "SELECT g.guarantor_id, g.pledge_amount, COALESCE(c.score, 50)
               FROM public.loan_guarantors g
               LEFT JOIN public.credit_scores c ON c.user_id = g.guarantor_id
              WHERE g.loan_id = $1 AND g.status = 'accepted'
              ORDER BY g.guarantor_id",
        )
        .bind(p.loan_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| db_err(e, "guarantor stakes"))?;

        let weights: Vec<i64> = balances.iter().map(|(_, balance)| *balance).collect();
        let stakes: Vec<domain::GuarantorStake> = guarantors
            .iter()
            .map(|(_, pledge, score)| domain::GuarantorStake { pledge: *pledge, score: *score })
            .collect();
        let booked = domain::book_interest(interest_total, &rules.params.interest_split, &weights, &stakes);
        let parts = booked.parts;

        for ((member, balance), amount) in balances.iter().zip(&booked.to_depositors) {
            if *amount > 0 {
                member_credits.push(MemberCredit {
                    user_id: *member, role: "depositor", weight: *balance, tier_share: None, amount: *amount,
                });
            }
        }
        for (((member, pledge, _), amount), tier) in
            guarantors.iter().zip(&booked.to_guarantors).zip(&booked.guarantor_tiers)
        {
            if *amount > 0 {
                member_credits.push(MemberCredit {
                    user_id: *member, role: "guarantor", weight: *pledge, tier_share: Some(*tier as i16), amount: *amount,
                });
            }
        }

        if principal_total > 0 {
            postings.push(Posting { account: "loans_receivable", amount: -principal_total });
        }
        for (account, amount) in [
            ("platform_earnings", parts.platform),
            ("reserve_fund", parts.reserve),
            ("recovery_fund", parts.recovery_fund),
            // Depositors' and guarantors' interest is owed to members, like the
            // excess below; each gets a lot of their own after the event.
            ("member_deposits", parts.depositors + parts.guarantor),
        ] {
            if amount > 0 {
                postings.push(Posting { account, amount: -amount });
            }
        }
        if excess > 0 {
            postings.push(Posting { account: "member_deposits", amount: -excess });
        }
        if interest_total > 0 {
            let pool_balance: i64 = weights.iter().sum();
            let pledged_total: i64 = guarantors.iter().map(|(_, pledge, _)| *pledge).sum();
            booked_split = Some((parts, pool_balance, pledged_total));
        }
        None
    };

    // Before the ledger event: the 041 trigger refuses a repayment event that
    // doesn't match a claimed intent carrying this capture reference.
    intents::consume(&mut tx, claimed.id, &captured.capture_id, provider_fee).await?;

    let event_id = commit_event(
        &mut tx,
        EventDraft {
            kind: "repayment_received",
            user_id: Some(user_id),
            loan_id: Some(p.loan_id),
            deposit_id: None,
            rail_ref: Some(captured.capture_id.clone()),
            payload: serde_json::json!({
                "rail": rail,
                "received": received, "applies": applies,
                "fee_paid": fee_paid, "provider_fee": provider_fee,
                "interest": interest_total,
                "principal": principal_total, "excess": excess
            }),
            actor_id: Some(user_id),
        },
        &postings,
    )
    .await
    .map_err(|e| ledger_err(e, "repayment"))?;

    if let Some((parts, pool_balance, pledged_total)) = booked_split {
        sqlx::query(
            "INSERT INTO public.interest_splits
                (event_id, loan_id, policy_version, interest,
                 platform, reserve, depositors, guarantor, recovery_fund, guarantor_share,
                 pool_balance, pledged_total)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NULL, $10, $11)",
        )
        .bind(event_id)
        .bind(p.loan_id)
        .bind(rules.id)
        .bind(interest_total)
        .bind(parts.platform)
        .bind(parts.reserve)
        .bind(parts.depositors)
        .bind(parts.guarantor)
        .bind(parts.recovery_fund)
        .bind(pool_balance)
        .bind(pledged_total)
        .execute(&mut *tx)
        .await
        .map_err(|e| db_err(e, "record interest split"))?;
    }

    // Every member's interest becomes withdrawable money: one new lot per
    // member per payment, holding their depositor and guarantor slices
    // together. New lots rather than topping up existing ones, so this never
    // takes a row lock another handler could be holding (see
    // `lots::deposit_balances`). The lots sum to the member_deposits credit
    // above because `book_interest`'s slices sum to their parts exactly, and
    // each slice is recorded with where it came from.
    let mut lots_to_credit: Vec<(Uuid, i64)> = Vec::new();
    for credit in &member_credits {
        match lots_to_credit.iter_mut().find(|(m, _)| *m == credit.user_id) {
            Some((_, total)) => *total += credit.amount,
            None => lots_to_credit.push((credit.user_id, credit.amount)),
        }
        sqlx::query(
            "INSERT INTO public.member_interest (event_id, loan_id, user_id, role, weight, tier_share, amount)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(event_id)
        .bind(p.loan_id)
        .bind(credit.user_id)
        .bind(credit.role)
        .bind(credit.weight)
        .bind(credit.tier_share)
        .bind(credit.amount)
        .execute(&mut *tx)
        .await
        .map_err(|e| db_err(e, "record member interest"))?;
    }
    for (member, amount) in lots_to_credit {
        sqlx::query("INSERT INTO public.deposits (user_id, amount, badge) VALUES ($1, $2, 'available')")
            .bind(member)
            .bind(amount)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_err(e, "credit member interest"))?;
    }

    // The borrower-facing payment record (migration 023): the ledger event
    // above is the books, this is the "here is every payment you made" row the
    // Pay page reads. The ledger's unique rail_ref already refused any double
    // credit, so by here this capture is known-unique.
    sqlx::query(
        "INSERT INTO public.loan_payments
            (loan_id, user_id, amount_received, interest_paid, principal_paid, excess, rail_ref, paid_at,
             fee_paid, provider_fee)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(p.loan_id)
    .bind(user_id)
    // What was applied to the loan; the fee paid on top is its own column.
    .bind(applies)
    .bind(interest_total)
    .bind(principal_total)
    .bind(excess)
    .bind(&captured.capture_id)
    .bind(now)
    .bind(fee_paid)
    .bind(provider_fee)
    .execute(&mut *tx)
    .await
    .map_err(|e| db_err(e, "record payment"))?;

    // Overpay becomes a withdrawable deposit lot instead of vanishing. On a
    // settlement `reconcile::settle` has already made this lot as the last step
    // of its distribution, so making a second one here would hand the borrower
    // their overpayment twice.
    if excess > 0 && !settling {
        sqlx::query("INSERT INTO public.deposits (user_id, amount, badge) VALUES ($1, $2, 'available')")
            .bind(user_id)
            .bind(excess)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_err(e, "excess lot"))?;
    }

    // A settling loan's principal was written down to zero by recovery, so
    // there is nothing left to reduce — subtracting the payment would take the
    // column negative and trip its own `>= 0` check. The schedule above is what
    // records the settlement's progress on a reopened loan.
    let outstanding_after = if settling { 0 } else { outstanding_before - principal_total };
    if !settling {
        sqlx::query("UPDATE public.loans SET principal_outstanding = $1, updated_at = $2 WHERE id = $3")
            .bind(outstanding_after)
            .bind(now)
            .bind(p.loan_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_err(e, "update outstanding"))?;

        // Principal came home -> that much funded deposit is withdrawable
        // again. Not on a settlement: recovery already released the savers'
        // lots when it settled the default, and releasing them a second time
        // would unfreeze deposits backing somebody else's loan.
        // Unlocked in proportion to the principal returned, pro-rata across
        // the members funding it, with the final payment unlocking the rest.
        if principal_total > 0 {
            let locked = lots::locked_funding(&mut tx, p.loan_id).await?;
            let release = domain::funding_to_release(locked, principal_total, outstanding_before);
            lots::release_funding_pro_rata(&mut tx, p.loan_id, release).await?;
        }
    }

    // A settlement never closes the loan by itself. Arrears reaching zero is
    // the *precondition* for reconciling, not the act — an administrator has to
    // accept it (`reconcile::mark_paid`), which is also what returns the credit
    // penalty and queues the on-chain outcome. Closing here would strand the
    // loan as `closed` with the borrower's score still on the floor.
    let fully_paid = !settling
        && outstanding_after == 0
        && schedule.iter().all(|r| r.interest_paid >= r.interest_due);

    let loan_status = if fully_paid {
        sqlx::query("UPDATE public.loans SET status = 'closed', closed_at = $1, updated_at = $1 WHERE id = $2")
            .bind(now)
            .bind(p.loan_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_err(e, "close loan"))?;

        // Collateral goes home, whatever shape it took.
        lots::release_loan_lots(&mut tx, p.loan_id, &["collateral", "pledged"]).await?;
        sqlx::query(
            "UPDATE public.loan_guarantors SET status = 'released', updated_at = $1
              WHERE loan_id = $2 AND status = 'accepted'",
        )
        .bind(now)
        .bind(p.loan_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| db_err(e, "release guarantors"))?;

        // Keyed off the position, not the product: since the 50% rule a
        // guarantor loan can carry part of the borrower's own half in coins,
        // and those have to come home on close exactly like a pure collateral
        // loan's. Gating on `product == "xlm_collateral"` here would leave a
        // guarantor borrower's XLM locked in the vault forever.
        {
            // The position stays `locked` until the chain says otherwise (030).
            // It used to flip to 'released' right here, which claimed coins had
            // gone home while they were still sitting in the vault — the DB and
            // the contract disagreeing, with nothing to reconcile them. The
            // lock path has always waited for Horizon; so does this one now.
            // `actions::confirm` moves the status when the release is verified.
            let collateral_id: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM public.xlm_collateral
                  WHERE loan_id = $1 AND status = 'locked'",
            )
            .bind(p.loan_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| db_err(e, "collateral position"))?;
            if let Some(cid) = collateral_id {
                // Two steps, in this order: the contract refuses to release
                // coins from a position with no repayment recorded against
                // it, so the outcome goes on-chain as its own transaction
                // before the money moves. The queue is ordered by id, which
                // is what keeps the pair the right way round.
                sqlx::query(
                    "INSERT INTO public.collateral_actions (collateral_id, action)
                     VALUES ($1, 'mark_repaid'), ($1, 'release')",
                )
                .bind(cid)
                .execute(&mut *tx)
                .await
                .map_err(|e| db_err(e, "queue release"))?;
            }
        }

        // Track record moves on real behavior (D5): repaid in full = +score,
        // logged like every other score change.
        let old_score: Option<i16> = sqlx::query_scalar(
            "SELECT score FROM public.credit_scores WHERE user_id = $1 FOR UPDATE",
        )
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| db_err(e, "score read"))?;
        if let Some(old_score) = old_score {
            let new_score = (old_score + SCORE_BUMP_ON_CLOSE).min(150);
            if new_score != old_score {
                sqlx::query("UPDATE public.credit_scores SET score = $1, updated_at = $2 WHERE user_id = $3")
                    .bind(new_score)
                    .bind(now)
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| db_err(e, "bump score"))?;
                sqlx::query(
                    "INSERT INTO public.credit_score_log (user_id, old_score, new_score, actor_id, reason)
                     VALUES ($1, $2, $3, NULL, $4)",
                )
                .bind(user_id)
                .bind(old_score)
                .bind(new_score)
                .bind(format!("loan {} repaid in full", p.loan_id))
                .execute(&mut *tx)
                .await
                .map_err(|e| db_err(e, "score log"))?;
            }
        }

        commit_event(
            &mut tx,
            EventDraft {
                kind: "loan_closed",
                user_id: Some(user_id),
                loan_id: Some(p.loan_id),
                deposit_id: None,
                rail_ref: None,
                payload: serde_json::json!({ "product": product }),
                actor_id: Some(user_id),
            },
            &[],
        )
        .await
        .map_err(|e| ledger_err(e, "loan_closed"))?;
        "closed"
    } else if settling {
        "reconciling"
    } else {
        "active"
    };

    // What is still owed after this payment — the number the borrower is
    // working down, and the one an admin needs at zero before they can accept
    // the settlement.
    let arrears_after = if settling {
        reconcile::arrears(&mut tx, p.loan_id).await?
    } else {
        0
    };

    tx.commit().await.map_err(|e| db_err(e, "commit repay"))?;
    if let Some(split) = &settlement {
        tracing::info!(
            %user_id, loan_id = %p.loan_id, received,
            guarantors = split.to_guarantors, reserve = split.to_reserve,
            borrower = split.to_borrower, arrears_after,
            "default settlement received"
        );
    } else {
        tracing::info!(%user_id, loan_id = %p.loan_id, received, interest_total, principal_total, "repayment received");
    }

    Ok(Json(RepayResponse {
        amount_received: applies,
        fee_paid,
        interest_paid: interest_total,
        principal_paid: principal_total,
        // On a settlement the "excess" the schedule allocation computed is not
        // what reached the borrower — `settle` distributes to the guarantors
        // and the reserve first, and only what survives that becomes their lot.
        excess_to_deposit: settlement.as_ref().map_or(excess, |s| s.to_borrower),
        principal_outstanding: outstanding_after,
        loan_status,
        message: if loan_status == "closed" {
            "Loan fully repaid — collateral released and your credit score just went up"
        } else if settling && arrears_after == 0 {
            "Arrears cleared — an administrator will confirm the settlement and restore your standing"
        } else if settling {
            "Payment received and applied to your arrears"
        } else {
            "Payment received and applied"
        },
    }))
}
