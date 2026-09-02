//! GET  /paypal/account    — what's linked, and whether payouts can run
//! POST /paypal/disconnect — unlink it
//!
//! The email is masked before it leaves the engine. It exists so a member can
//! recognise which of their PayPal accounts is linked, not so a screen (or
//! anything reading a screenshot) can read it back in full.

use axum::{Extension, Json, http::HeaderMap};
use chrono::Utc;
use serde::Serialize;
use sqlx::PgPool;

use crate::api::lending::shared::db_err;
use crate::api::users::shared::{E, require_verified_user};
use crate::infra::paypal;

#[derive(Serialize)]
pub struct AccountStatus {
    pub connected: bool,
    /// "j•••@gmail.com" — enough to recognise, not enough to reuse.
    pub email_masked: Option<String>,
    /// PayPal's own word on whether the account is verified. An unverified
    /// account can still receive, but may hold the funds until it is.
    pub verified: bool,
    pub connected_at: Option<i64>,
    /// False when this deployment can't run the connect flow — no PayPal
    /// credentials, or no registered return URL. Both are required, so this
    /// tracks `can_connect`, not `is_configured`: reporting ready on
    /// credentials alone is what made the button fail on click.
    pub paypal_ready: bool,
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

    let row: Option<(String, bool, i64)> = sqlx::query_as(
        "SELECT email, verified, connected_at FROM public.paypal_accounts
          WHERE user_id = $1 AND status = 'active'",
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| db_err(e, "paypal account"))?;

    Ok(Json(match row {
        Some((email, verified, connected_at)) => AccountStatus {
            connected: true,
            email_masked: Some(mask(&email)),
            verified,
            connected_at: Some(connected_at),
            paypal_ready: paypal::can_connect(),
        },
        None => AccountStatus {
            connected: false,
            email_masked: None,
            verified: false,
            connected_at: None,
            paypal_ready: paypal::can_connect(),
        },
    }))
}

#[derive(Serialize)]
pub struct DisconnectResponse {
    pub message: &'static str,
}

/// Unlinks the account. Payouts already in flight keep the destination they
/// were created with — `payouts.payer_id` is pinned at request time — so
/// disconnecting can never redirect money that is already moving.
pub async fn disconnect(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
) -> Result<Json<DisconnectResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    sqlx::query(
        "UPDATE public.paypal_accounts SET status = 'disconnected', updated_at = $1
          WHERE user_id = $2 AND status = 'active'",
    )
    .bind(Utc::now().timestamp())
    .bind(user_id)
    .execute(&pool)
    .await
    .map_err(|e| db_err(e, "paypal disconnect"))?;

    tracing::info!(%user_id, "paypal account disconnected");
    Ok(Json(DisconnectResponse {
        message: "PayPal disconnected — connect an account again before withdrawing",
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
