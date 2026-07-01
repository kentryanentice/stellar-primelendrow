use argon2::{
    Algorithm, Argon2, Params, PasswordHasher, Version,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::{Extension, Json, http::StatusCode};
use chrono::Utc;
use rand::RngExt;
use serde::Deserialize;
use sqlx::PgPool;

use super::shared::{E, MessageResponse, hash_code, is_strong_password, is_valid_email};
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
    let hash = argon2
        .hash_password(p.password.as_bytes(), &salt)
        .unwrap()
        .to_string();
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
        tracing::warn!(email = %p.email, "register failed: email already exists");
        return Err((StatusCode::CONFLICT, "Registration failed"));
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

    if let Some(existing) = sqlx::query!(
        "SELECT expires_at, created_at FROM public.verification_codes WHERE email = $1 FOR UPDATE",
        p.email
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Registration failed")
    })? {
        if existing.expires_at > now && existing.created_at > now - 60 {
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                "Please wait 1 minute before requesting another code",
            ));
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

    mailer::send_code(&p.email, &code).await.map_err(|e| {
        tracing::error!("mailer: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to send email")
    })?;

    tracing::info!(email = %p.email, "verification code sent");
    Ok((
        StatusCode::CREATED,
        Json(MessageResponse {
            message: "Verification code sent",
        }),
    ))
}
