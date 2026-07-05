use argon2::{
    Algorithm, Argon2, Params, PasswordHasher, Version,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::{Extension, Json, http::StatusCode};
use chrono::Utc;
use rand::RngExt;
use serde::Deserialize;
use sqlx::PgPool;

use super::shared::{
    E, MessageResponse, hash_code, hash_permit, is_strong_password, is_valid_email, verify_code,
};
use crate::api::mailer;
use crate::api::verified::Verified;

#[derive(Deserialize)]
struct RequestInput {
    email: String,
}

#[derive(Deserialize)]
struct ConfirmInput {
    email: String,
    code: String,
    password: String,
}

/// POST /auth/password-reset/request
///
/// Always returns 200 with the same message regardless of whether the email
/// is registered, to avoid leaking which addresses have accounts. The OTP is
/// only generated and emailed when the account actually exists.
pub async fn request(
    Extension(pool): Extension<PgPool>,
    Verified(msg, _): Verified,
) -> Result<(StatusCode, Json<MessageResponse>), E> {
    let mut p: RequestInput = serde_json::from_slice(&msg)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid payload JSON"))?;
    p.email = p.email.trim().to_ascii_lowercase();

    if !is_valid_email(&p.email) || p.email.len() > 255 {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Invalid email"));
    }

    let now = Utc::now().timestamp();

    let user = sqlx::query!("SELECT id FROM public.users WHERE email = $1", p.email)
        .fetch_optional(&pool)
        .await
        .map_err(|e| {
            tracing::error!("DB: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Reset failed")
        })?;

    if user.is_none() {
        return Ok((
            StatusCode::OK,
            Json(MessageResponse {
                message: "If that email is registered, a code has been sent",
            }),
        ));
    }

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Reset failed")
    })?;

    if let Some(existing) = sqlx::query!(
        "SELECT created_at FROM public.password_reset_codes WHERE email = $1 FOR UPDATE",
        p.email
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Reset failed")
    })? {
        if existing.created_at > now - 60 {
            // Silently accept: same generic 200 as every other outcome, no
            // new code, no email. A 429 here only ever fired for registered
            // emails (reset rows exist only for real accounts), which made
            // the cooldown an account-existence oracle. The earlier code is
            // still valid, so a fast resend loses nothing.
            return Ok((
                StatusCode::OK,
                Json(MessageResponse {
                    message: "If that email is registered, a code has been sent",
                }),
            ));
        }
        sqlx::query!(
            "DELETE FROM public.password_reset_codes WHERE email = $1",
            p.email
        )
        .execute(&mut *tx)
        .await
        .ok();
    }

    let code = format!("{:06}", rand::rng().random_range(0..1_000_000u32));
    let expires_at = now + 600;

    sqlx::query!(
        "INSERT INTO public.password_reset_codes (email, code, expires_at, created_at)
         VALUES ($1, $2, $3, $4)",
        p.email,
        hash_code(&code),
        expires_at,
        now
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Reset failed")
    })?;

    tx.commit().await.map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Reset failed")
    })?;

    // Sent off-task, mirroring register: the mailer round-trip only happens
    // for real accounts, so awaiting it here was a response-latency oracle
    // for which emails are registered. Failures are logged; the user
    // recovers with the resend button.
    let email = p.email.clone();
    tokio::spawn(async move {
        if let Err(e) = mailer::send_code(&email, &code).await {
            tracing::error!("mailer: {e}");
        }
    });

    tracing::info!(email = %p.email, "password reset code sent");
    Ok((
        StatusCode::OK,
        Json(MessageResponse {
            message: "If that email is registered, a code has been sent",
        }),
    ))
}

/// POST /auth/password-reset/confirm
pub async fn confirm(
    Extension(pool): Extension<PgPool>,
    Verified(msg, _): Verified,
) -> Result<(StatusCode, Json<MessageResponse>), E> {
    let mut p: ConfirmInput = serde_json::from_slice(&msg)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid payload JSON"))?;
    p.email = p.email.trim().to_ascii_lowercase();

    if p.code.len() != 6 || !p.code.chars().all(|c| c.is_ascii_digit()) {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Invalid code format"));
    }
    if !is_strong_password(&p.password) {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Password too weak"));
    }

    let now = Utc::now().timestamp();
    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Reset failed")
    })?;

    sqlx::query!(
        "DELETE FROM public.password_reset_codes WHERE expires_at <= $1",
        now
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Reset failed")
    })?;

    let prc = sqlx::query!(
        "SELECT code, expires_at, failed_attempts
         FROM public.password_reset_codes WHERE email = $1 FOR UPDATE",
        p.email
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Reset failed")
    })?
    // same message as a wrong code: reset rows only exist for real accounts,
    // so "no pending reset" after a generic /request response was an
    // account-enumeration oracle
    .ok_or((StatusCode::UNPROCESSABLE_ENTITY, "Incorrect reset code"))?;

    if prc.expires_at <= now {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Reset code has expired"));
    }

    if prc.failed_attempts >= 5 {
        sqlx::query!(
            "DELETE FROM public.password_reset_codes WHERE email = $1",
            p.email
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!("DB: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Reset failed")
        })?;
        tx.commit().await.ok();
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "Too many incorrect attempts. Please request a new code.",
        ));
    }

    if !verify_code(&p.code, &prc.code) {
        let _ = sqlx::query!(
            "UPDATE public.password_reset_codes SET failed_attempts = failed_attempts + 1 WHERE email = $1",
            p.email
        )
        .execute(&mut *tx).await;
        tx.commit().await.ok();
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Incorrect reset code"));
    }

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

    let updated = sqlx::query!(
        "UPDATE public.users
            SET password_hash         = $1,
                updated_at            = $2,
                failed_login_attempts = 0,
                lockout_until         = 0
          WHERE email = $3",
        hash,
        now,
        p.email
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Reset failed")
    })?;

    if updated.rows_affected() == 0 {
        // only reachable if the account vanished mid-flow; keep it generic
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Reset failed"));
    }

    sqlx::query!(
        "DELETE FROM public.password_reset_codes WHERE email = $1",
        p.email
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Reset failed")
    })?;

    sqlx::query!(
        "DELETE FROM public.sessions
          WHERE user_id = (SELECT id FROM public.users WHERE email = $1)",
        p.email
    )
    .execute(&mut *tx)
    .await
    .ok();

    tx.commit().await.map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Reset failed")
    })?;

    tracing::info!(email = %p.email, "password reset successful");
    Ok((
        StatusCode::OK,
        Json(MessageResponse {
            message: "Password updated",
        }),
    ))
}
