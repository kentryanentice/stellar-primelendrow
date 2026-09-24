//! GET /pool — the single read that drives the whole Lending page: the
//! pool's honest utilization numbers, the caller's own four numbers with
//! their lots, and the engine's parameters (rules, fx, contract) so the
//! frontend never has to invent any of them (Lesson 3: the UI is a window,
//! never a calculator).

use axum::{Extension, Json, http::HeaderMap};
use serde::Serialize;
use sqlx::PgPool;

use super::domain;
use super::ledger::{free_cash, retained_funds, unwithdrawn_proceeds};
use super::policy::{self, PolicyParams};
use super::pricing;
use super::shared::db_err;
use crate::api::users::shared::{E, require_verified_user};
use crate::infra::{paypal, stellar, stripe};

#[derive(Serialize)]
pub struct PoolStats {
    /// Members' deposits — every lot except borrowers' loan proceeds, which
    /// are `proceeds_waiting` instead.
    pub total_deposits: i64,
    /// Members' cash the pool can lend or pay out right now. Excludes the
    /// retained funds and borrowers' waiting proceeds, so that
    /// `out_on_loans + cash_available == total_deposits` to the centavo, less
    /// only seized collateral still held as treasury assets and any proceeds
    /// already moved out of `available` (pledged, say).
    pub cash_available: i64,
    /// The platform fee, lending reserve and recovery fund the interest split
    /// has built up, and any payment-fee variance: cash the platform holds,
    /// but held rather than lent, and not in `cash_available`.
    pub pool_funds: i64,
    /// Loan proceeds in borrowers' balances, not yet withdrawn (050): held for
    /// the borrower, so in neither the pool size nor `cash_available`.
    pub proceeds_waiting: i64,
    pub out_on_loans: i64,
    pub active_loans: i64,
    /// 0..100, rounded — how much of the pool's money is out on loans.
    pub utilization_pct: i64,
    /// The same in basis points, for display to one decimal: a pool 1.55%
    /// working should not read as 1%, nor 0.9% as idle.
    pub utilization_bps: i64,
    pub interest: InterestCollected,
}

/// All interest the pool has collected, and where it actually went: the sum
/// of every recorded split (037), not the published rule applied after the
/// fact. `parts` always sums to `total`, because every row does.
#[derive(Serialize)]
pub struct InterestCollected {
    pub total: i64,
    pub payments: i64,
    pub parts: domain::InterestParts,
}

#[derive(Serialize)]
pub struct MyFunds {
    pub available: i64,
    pub lent: i64,
    pub collateral: i64,
    pub pledged: i64,
    /// The part of `available` that is this member's own loan proceeds (050):
    /// theirs to withdraw, but not part of their stake in the pool.
    pub proceeds: i64,
    pub score: i16,
    /// What this member has been paid out of repayments' interest (039),
    /// split by why: their deposit balance in the pool, and loans they
    /// guarantee.
    pub interest_earned: MyInterest,
    /// The member's AML deposit limits (042) and how much of them is left.
    pub deposit_limits: Option<super::intents::DepositStatus>,
}

#[derive(Serialize, Default)]
pub struct MyInterest {
    pub total: i64,
    pub as_depositor: i64,
    pub as_guarantor: i64,
    /// Repayments that paid this member anything.
    pub payments: i64,
}

#[derive(Serialize)]
pub struct Params {
    pub policy: PolicyParams,
    /// The agreed rate on its own — every existing caller reads this.
    pub fx_centavos_per_xlm: i64,
    /// ...and the same rate with its provenance: which feeds agreed, when
    /// they were read, and whether this is a live agreement or a held one.
    pub fx: pricing::Priced,
    pub collateral_contract: Option<String>,
    pub paypal_ready: bool,
    /// Whether the card rail is switched on (`STRIPE_SECRET_KEY` set). The
    /// pages hide every Stripe control when it isn't, so a PayPal-only
    /// deployment is just a missing key — no code change either way.
    pub stripe_ready: bool,
    /// The published worked example, split by the engine so the screen only
    /// draws it.
    pub split_example: SplitExample,
}

/// The SOW's worked example: ₱1,000 at 2%/mo, first payment's interest ₱20.00.
const EXAMPLE_INTEREST: i64 = 2_000;

#[derive(Serialize)]
pub struct TierExample {
    pub min_score: i16,
    pub max_score: i16,
    pub share: i64,
    pub parts: domain::InterestParts,
}

#[derive(Serialize)]
pub struct SplitExample {
    pub interest: i64,
    pub no_guarantor: domain::InterestParts,
    pub tiers: Vec<TierExample>,
}

fn split_example(params: &PolicyParams) -> SplitExample {
    let split = &params.interest_split;
    SplitExample {
        interest: EXAMPLE_INTEREST,
        no_guarantor: domain::split_interest_parts(EXAMPLE_INTEREST, split, None),
        tiers: split
            .guarantor_tiers
            .iter()
            .map(|t| TierExample {
                min_score: t.min_score,
                max_score: t.max_score,
                share: t.share,
                parts: domain::split_interest_parts(EXAMPLE_INTEREST, split, Some(t.share)),
            })
            .collect(),
    }
}

#[derive(Serialize)]
pub struct PoolResponse {
    pub pool: PoolStats,
    pub me: MyFunds,
    pub params: Params,
}

pub async fn summary(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
) -> Result<Json<PoolResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    let rules = policy::active(&pool).await?;
    let fx = pricing::for_display(&pool).await?;

    // ::BIGINT everywhere SUM appears: Postgres widens SUM(BIGINT) to NUMERIC,
    // which sqlx refuses to decode as i64.
    // The pool is what members have put in to lend. Borrowers' proceeds
    // waiting to be withdrawn (050) are borrowed money sitting in their
    // balances — never lent on, earning nothing — so they are reported on
    // their own, as `proceeds_waiting`, and not as pool.
    let total_deposits: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0)::BIGINT FROM public.deposits
          WHERE origin <> 'loan_proceeds'",
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| db_err(e, "pool totals"))?;

    // Out on loans is the ledger's receivable, not a sum over `active` loans:
    // a default still in recovery, or one reopened for settlement, is money
    // still owed to the pool, and counting only running loans would drop it
    // from the page while the books still carry it.
    let out_on_loans: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0)::BIGINT FROM public.ledger_postings
          WHERE account = 'loans_receivable'",
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| db_err(e, "loan totals"))?;

    // "Available" means available to lend, netted exactly as `disburse` nets
    // it before lending: withdrawals promised and not yet paid out (029),
    // borrowers' proceeds waiting to be withdrawn (050), and the platform,
    // reserve and recovery funds, which are held rather than lent.
    let proceeds_waiting = unwithdrawn_proceeds(&pool)
        .await
        .map_err(|e| db_err(e, "proceeds waiting"))?;
    let pool_funds = retained_funds(&pool)
        .await
        .map_err(|e| db_err(e, "retained funds"))?;
    let cash_available = free_cash(&pool)
        .await
        .map_err(|e| db_err(e, "cash balance"))?
        - proceeds_waiting
        - pool_funds;

    let active_loans: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM public.loans WHERE status = 'active'")
            .fetch_one(&pool)
            .await
            .map_err(|e| db_err(e, "active loans"))?;

    // Rounded to the nearest basis point rather than floored to the percent.
    let working = out_on_loans + cash_available;
    let utilization_bps = if working > 0 { (out_on_loans * 10_000 + working / 2) / working } else { 0 };
    let utilization_pct = (utilization_bps + 50) / 100;

    let (total, payments, platform, reserve, depositors, guarantor, recovery_fund):
        (i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(interest), 0)::BIGINT, COUNT(*),
                COALESCE(SUM(platform), 0)::BIGINT, COALESCE(SUM(reserve), 0)::BIGINT,
                COALESCE(SUM(depositors), 0)::BIGINT, COALESCE(SUM(guarantor), 0)::BIGINT,
                COALESCE(SUM(recovery_fund), 0)::BIGINT
           FROM public.interest_splits",
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| db_err(e, "interest totals"))?;
    let interest = InterestCollected {
        total,
        payments,
        parts: domain::InterestParts { platform, reserve, depositors, guarantor, recovery_fund },
    };

    // The four running totals, grouped in the database rather than summed by
    // looping the caller's full lot list — the list itself now lives behind
    // its own paginated endpoint (deposits_list) and this response no longer
    // fetches it.
    let badge_totals: Vec<(String, i64)> = sqlx::query_as(
        "SELECT badge, COALESCE(SUM(amount), 0)::BIGINT
           FROM public.deposits
          WHERE user_id = $1
          GROUP BY badge",
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| db_err(e, "my badge totals"))?;
    let proceeds: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount), 0)::BIGINT FROM public.deposits
          WHERE user_id = $1 AND badge = 'available' AND origin = 'loan_proceeds'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| db_err(e, "my proceeds"))?;

    let mut me = MyFunds {
        available: 0,
        lent: 0,
        collateral: 0,
        pledged: 0,
        proceeds,
        score: 50,
        interest_earned: MyInterest::default(),
        deposit_limits: None,
    };
    for (badge, amount) in badge_totals {
        match badge.as_str() {
            "available" => me.available = amount,
            "lent" => me.lent = amount,
            "collateral" => me.collateral = amount,
            "pledged" => me.pledged = amount,
            _ => {}
        }
    }

    me.score = sqlx::query_scalar("SELECT score FROM public.credit_scores WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| db_err(e, "credit score"))?
        .unwrap_or(50);

    let (as_depositor, as_guarantor, payments): (i64, i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(amount) FILTER (WHERE role = 'depositor'), 0)::BIGINT,
                COALESCE(SUM(amount) FILTER (WHERE role = 'guarantor'), 0)::BIGINT,
                COUNT(DISTINCT event_id)
           FROM public.member_interest
          WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| db_err(e, "my interest"))?;
    let mut conn = pool.acquire().await.map_err(|e| db_err(e, "acquire for deposit limits"))?;
    me.deposit_limits = Some(super::intents::deposit_status(&mut conn, user_id, &rules.params).await?);
    drop(conn);
    me.interest_earned = MyInterest { total: as_depositor + as_guarantor, as_depositor, as_guarantor, payments };

    Ok(Json(PoolResponse {
        pool: PoolStats {
            total_deposits,
            cash_available,
            pool_funds,
            proceeds_waiting,
            out_on_loans,
            active_loans,
            utilization_pct,
            utilization_bps,
            interest,
        },
        me,
        params: Params {
            split_example: split_example(&rules.params),
            policy: rules.params,
            fx_centavos_per_xlm: fx.centavos_per_xlm,
            fx,
            collateral_contract: stellar::contract_id(),
            paypal_ready: paypal::is_configured(),
            stripe_ready: stripe::is_configured(),
        },
    }))
}
