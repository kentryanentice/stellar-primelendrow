//! GET  /stripe/account    — what's linked, and whether transfers can run
//! POST /stripe/disconnect — unlink it
//!
//! The mirror of `api::paypal::account`. The email is masked before it leaves
//! the engine, by the same `mask` this codebase already applies to a PayPal
//! address: it exists so a member can recognise which account is linked, not
//! so a screen (or anything reading a screenshot) can read it back in full.

use axum::{Extension, Json, http::HeaderMap};
use chrono::Utc;
use serde::Serialize;
use sqlx::PgPool;

use crate::api::lending::shared::db_err;
use crate::api::users::shared::{E, require_verified_user};
use crate::infra::stripe;

#[derive(Serialize)]
pub struct AccountStatus {
    /// True only when the account can actually receive money. An onboarding
    /// that was started and abandoned is NOT connected — reporting it as such
    /// is how a member finds out at withdrawal time instead of now.
    pub connected: bool,
    /// True when a row exists but Stripe hasn't cleared it yet. The Settings
    /// card uses this to offer "Finish setting up" rather than "Connect".
    pub onboarding: bool,
    /// "j•••@gmail.com" — enough to recognise, not enough to reuse.
    pub email_masked: Option<String>,
    /// Stripe's own word on whether money may be sent to this account.
    pub payouts_enabled: bool,
    pub connected_at: Option<i64>,
    /// False when this deployment can't run onboarding — no secret key, or a
    /// missing callback URL. Both are required, so this tracks `can_connect`,
    /// not `is_configured`.
    pub stripe_ready: bool,
}

/// `juan.delacruz@gmail.com` -> `j•••@gmail.com`.
fn mask(email: &str) -> String {
    match email.split_once('@') {
        Some((user, domain)) => {
            let first = user.chars().next().unwrap_or('•');
            format!("{first}•••@{domain}")
        }
        None => "•••".to_string(),
    }
}

pub async fn status(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
) -> Result<Json<AccountStatus>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    let row: Option<(String, bool, bool, i64)> = sqlx::query_as(
        "SELECT email, payouts_enabled, details_submitted, connected_at
           FROM public.stripe_accounts
          WHERE user_id = $1 AND status = 'active'",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| db_err(e, "stripe account"))?;

    Ok(Json(match row {
        Some((email, payouts_enabled, details_submitted, connected_at)) => AccountStatus {
            connected: payouts_enabled,
            // Started but not finished, or finished but not yet cleared —
            // either way there is something to resume rather than begin.
            onboarding: !payouts_enabled,
            email_masked: (!email.is_empty()).then(|| mask(&email)),
            payouts_enabled,
            connected_at: Some(connected_at),
            stripe_ready: stripe::can_connect(),
            // `details_submitted` is read but not published on its own: from
            // the member's side the only question that matters is whether the
            // money can arrive, and `payouts_enabled` answers it.
        }
        .with_details(details_submitted),
        None => AccountStatus {
            connected: false,
            onboarding: false,
            email_masked: None,
            payouts_enabled: false,
            connected_at: None,
            stripe_ready: stripe::can_connect(),
        },
    }))
}

impl AccountStatus {
    /// A row whose form was never submitted is mid-onboarding no matter what
    /// else is true of it.
    fn with_details(mut self, details_submitted: bool) -> Self {
        if !details_submitted {
            self.onboarding = true;
            self.connected = false;
        }
        self
    }
}

#[derive(Serialize)]
pub struct DisconnectResponse {
    pub message: &'static str,
}

/// Unlinks the account. Payouts already in flight keep the destination they
/// were created with — `payouts.payer_id` is pinned at request time — so
/// disconnecting can never redirect money that is already moving.
///
/// The connected account itself is left standing at Stripe rather than
/// deleted: it may hold a balance or an obligation that is the member's, and
/// destroying it from here would be this platform deciding something about
/// their Stripe relationship that isn't ours to decide.
pub async fn disconnect(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
) -> Result<Json<DisconnectResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    sqlx::query(
        "UPDATE public.stripe_accounts SET status = 'disconnected', updated_at = $1
          WHERE user_id = $2 AND status = 'active'",
    )
    .bind(Utc::now().timestamp())
    .bind(user_id)
    .execute(&pool)
    .await
    .map_err(|e| db_err(e, "stripe disconnect"))?;

    tracing::info!(%user_id, "stripe account disconnected");
    Ok(Json(DisconnectResponse {
        message: "Stripe disconnected — connect an account again before withdrawing",
    }))
}

#[cfg(test)]
mod tests {
    use super::mask;

    #[test]
    fn masks_everything_but_the_shape() {
        assert_eq!(mask("juan.delacruz@gmail.com"), "j•••@gmail.com");
        assert_eq!(mask("a@b.ph"), "a•••@b.ph");
        assert_eq!(mask("not-an-email"), "•••");
    }
}
