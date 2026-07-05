use argon2::{
    Algorithm, Argon2, Params, PasswordHasher, PasswordVerifier, Version,
    password_hash::{PasswordHash, SaltString, rand_core::OsRng},
};
use axum::{Extension, Json, http::StatusCode};
use chrono::Utc;
use rand::RngExt;
use serde::Deserialize;
use sqlx::PgPool;

use super::shared::{E, MessageResponse, hash_code, hash_permit, is_strong_password, is_valid_email};
use crate::api::mailer;
use crate::api::verified::Verified;

#[derive(Deserialize)]
struct RegisterInput {
    name: String,
    email: String,
    password: String,
    access_token: Option<String>,
}

pub async fn register(
    Extension(pool): Extension<PgPool>,
    Verified(msg, _): Verified,
) -> Result<(StatusCode, Json<MessageResponse>), E> {
    let mut p: RegisterInput = serde_json::from_slice(&msg)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid payload JSON"))?;
    p.email = p.email.trim().to_ascii_lowercase();

    if !is_valid_email(&p.email) {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Invalid email"));
    }
    if p.name.is_empty() || p.name.len() > 255 {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Name too long"));
    }
    if p.email.len() > 255 {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Email too long"));
    }
    if !is_strong_password(&p.password) {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Password too weak"));
    }
    let access_token = p.access_token.unwrap_or_default().trim().to_string();
    if access_token.len() > 100 {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Invalid access token"));
    }

    let now = Utc::now().timestamp();
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(65536, 3, 4, None).expect("valid Argon2 params"),
    );
    let permit = hash_permit().await;
    let hash = argon2
        .hash_password(p.password.as_bytes(), &salt)
        .unwrap()
        .to_string();
    drop(permit);
    let code = format!("{:06}", rand::rng().random_range(0..1_000_000u32));
    let expires_at = now + 600;

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Registration failed")
    })?;

    if sqlx::query!("SELECT id FROM public.users WHERE email = $1", p.email)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!("DB: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Registration failed")
        })?
        .is_some()
    {
        // Indistinguishable from success so the register endpoint can't be
        // used to test which emails have accounts (409 here was an oracle).
        // No code is stored or emailed; a later /auth/verify just fails like
        // any wrong code. The password was already hashed above, so timing
        // stays comparable to the real path minus the mailer call.
        tracing::warn!(email = %p.email, "register attempt for existing email");
        return Ok((
            StatusCode::CREATED,
            Json(MessageResponse {
                message: "Verification code sent",
            }),
        ));
    }

    let access_token_opt: Option<String> = if access_token.is_empty() {
        None
    } else {
        Some(access_token.clone())
    };

    if let Some(ref token) = access_token_opt {
        let token_row = sqlx::query!(
            "SELECT token, reserved_email, reserved_at, redeemed_at, revoked_at
             FROM public.access_tokens WHERE token = $1 FOR UPDATE",
            token
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!("DB: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Registration failed")
        })?
        .ok_or((StatusCode::UNPROCESSABLE_ENTITY, "Invalid access token"))?;

        if token_row.revoked_at.is_some() {
            return Err((StatusCode::UNPROCESSABLE_ENTITY, "Invalid access token"));
        }
        if token_row.redeemed_at.is_some() {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                "Access token already used",
            ));
        }

        let token_expires: Option<i64> = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT expires_at FROM public.access_tokens WHERE token = $1",
        )
        .bind(token)
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(None);

        if token_expires.is_some_and(|exp| exp <= now) {
            return Err((StatusCode::UNPROCESSABLE_ENTITY, "Access token has expired"));
        }

        if let (Some(reserved_email), Some(reserved_at)) =
            (token_row.reserved_email.as_deref(), token_row.reserved_at)
            && reserved_at >= now - 600
            && reserved_email != p.email
        {
            return Err((StatusCode::CONFLICT, "Access token already reserved"));
        }

        sqlx::query!(
            "UPDATE public.access_tokens SET reserved_email = $1, reserved_at = $2 WHERE token = $3",
            p.email, now, token
        )
        .execute(&mut *tx).await
        .map_err(|e| { tracing::error!("DB: {e}"); (StatusCode::INTERNAL_SERVER_ERROR, "Registration failed") })?;
    }

    if let Some((pending_hash, pending_expires, pending_created)) =
        sqlx::query_as::<_, (String, i64, i64)>(
            "SELECT password_hash, expires_at, created_at
             FROM public.verification_codes WHERE email = $1 FOR UPDATE",
        )
        .bind(&p.email)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!("DB: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Registration failed")
        })?
    {
        if pending_expires > now && pending_created > now - 60 {
            // Silently accept: same generic success as the existing-account
            // path, no new code, no email. A 429 here only ever fired while a
            // pending (i.e. *unregistered*) signup existed — the mirror image
            // of the existing-email fake success — so together they formed an
            // account-existence oracle. The earlier code is still valid.
            tracing::info!(email = %p.email, "register attempt within resend cooldown");
            return Ok((
                StatusCode::CREATED,
                Json(MessageResponse {
                    message: "Verification code sent",
                }),
            ));
        }
        // While a pending registration is still live, only the original
        // registrant (proven by knowing its password — the resend button
        // re-posts the identical payload) may replace it. Otherwise anyone
        // could swap in their own password after the 60s cooldown and hijack
        // the account the moment the victim enters the fresher emailed code.
        // Answered with the same generic success as the existing-account
        // path so it can't be used to probe for pending registrations.
        if pending_expires > now {
            let permit = hash_permit().await;
            let same_password = PasswordHash::new(&pending_hash).is_ok_and(|parsed| {
                argon2
                    .verify_password(p.password.as_bytes(), &parsed)
                    .is_ok()
            });
            drop(permit);
            if !same_password {
                tracing::warn!(email = %p.email, "register attempt while a different pending registration exists");
                return Ok((
                    StatusCode::CREATED,
                    Json(MessageResponse {
                        message: "Verification code sent",
                    }),
                ));
            }
        }
        sqlx::query!(
            "DELETE FROM public.verification_codes WHERE email = $1",
            p.email
        )
        .execute(&mut *tx)
        .await
        .ok();
    }

    sqlx::query!(
        "INSERT INTO public.verification_codes
             (username, email, password_hash, code, expires_at, created_at, access_token)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
        p.name,
        p.email,
        hash,
        hash_code(&code),
        expires_at,
        now,
        access_token_opt
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Registration failed")
    })?;

    tx.commit().await.map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Registration failed")
    })?;

    // Sent off-task: the mailer's network round-trip was a timing oracle —
    // the existing-email path above (which sends nothing) answered measurably
    // faster than this one. Failures are logged, and the user recovers with
    // the resend button.
    let email = p.email.clone();
    tokio::spawn(async move {
        if let Err(e) = mailer::send_code(&email, &code).await {
            tracing::error!("mailer: {e}");
        }
    });

    tracing::info!(email = %p.email, "verification code sent");
    Ok((
        StatusCode::CREATED,
        Json(MessageResponse {
            message: "Verification code sent",
        }),
    ))
}
