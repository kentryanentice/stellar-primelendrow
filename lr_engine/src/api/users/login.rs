use argon2::{
    Algorithm, Argon2, Params, PasswordHasher, PasswordVerifier, Version,
    password_hash::{PasswordHash, SaltString, rand_core::OsRng},
};
use axum::{
    Extension, Json,
    http::{HeaderMap, HeaderValue, StatusCode, header::SET_COOKIE},
};
use chrono::Utc;
use serde::Deserialize;
use sqlx::PgPool;

use super::shared::{
    E, MAX_PASSWORD_LEN, SESSION_MAX_AGE, UserResponse, csrf_cookie, is_valid_email,
    new_csrf_token, session_cookie,
};
use crate::api::verified::Verified;

// Per-email lockout tuning: 5 bad attempts → 15-minute lockout.
const MAX_FAILED_ATTEMPTS: i32 = 5;
const LOCKOUT_SECONDS: i64 = 15 * 60;

#[derive(Deserialize)]
struct LoginInput {
    email: String,
    password: String,
}

pub async fn login(
    Extension(pool): Extension<PgPool>,
    Verified(msg, _): Verified,
) -> Result<(StatusCode, HeaderMap, Json<UserResponse>), E> {
    let mut p: LoginInput =
        serde_json::from_slice(&msg).map_err(|_| (StatusCode::BAD_REQUEST, "Invalid payload"))?;
    p.email = p.email.trim().to_ascii_lowercase();

    if !is_valid_email(&p.email) || p.email.len() > 255 || p.password.len() > MAX_PASSWORD_LEN {
        return Err((StatusCode::UNAUTHORIZED, "Invalid email or password"));
    }

    let now = Utc::now().timestamp();

    let row = match sqlx::query!(
        "SELECT id, username, email, password_hash, role, failed_login_attempts, lockout_until
         FROM public.users WHERE email = $1",
        p.email
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Login failed")
    })? {
        Some(row) => row,
        None => {
            // Burn a comparable amount of time hashing so a missing account
            // can't be distinguished from a wrong password by response latency.
            let argon2 = Argon2::new(
                Algorithm::Argon2id,
                Version::V0x13,
                Params::new(65536, 3, 4, None).expect("valid Argon2 params"),
            );
            let _ = argon2.hash_password(p.password.as_bytes(), &SaltString::generate(&mut OsRng));
            return Err((StatusCode::UNAUTHORIZED, "Invalid email or password"));
        }
    };

    if row.lockout_until > now {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "Too many failed attempts. Please try again later.",
        ));
    }

    let parsed = PasswordHash::new(&row.password_hash)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Login failed"))?;

    if Argon2::default()
        .verify_password(p.password.as_bytes(), &parsed)
        .is_err()
    {
        let new_count = row.failed_login_attempts + 1;
        if new_count >= MAX_FAILED_ATTEMPTS {
            let _ = sqlx::query!(
                "UPDATE public.users
                   SET failed_login_attempts = 0, lockout_until = $2
                 WHERE id = $1",
                row.id,
                now + LOCKOUT_SECONDS
            )
            .execute(&pool)
            .await;
            tracing::warn!(email = %p.email, "account locked after repeated failed logins");
        } else {
            let _ = sqlx::query!(
                "UPDATE public.users SET failed_login_attempts = $2 WHERE id = $1",
                row.id,
                new_count
            )
            .execute(&pool)
            .await;
        }
        return Err((StatusCode::UNAUTHORIZED, "Invalid email or password"));
    }

    if row.failed_login_attempts != 0 || row.lockout_until != 0 {
        let _ = sqlx::query!(
            "UPDATE public.users SET failed_login_attempts = 0, lockout_until = 0 WHERE id = $1",
            row.id
        )
        .execute(&pool)
        .await;
    }

    let _ = sqlx::query!(
        "DELETE FROM public.sessions WHERE user_id = $1 OR expires_at <= $2",
        row.id,
        now
    )
    .execute(&pool)
    .await;

    let session_id = uuid::Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO public.sessions (id, user_id, created_at, expires_at) VALUES ($1, $2, $3, $4)",
        session_id,
        row.id,
        now,
        now + SESSION_MAX_AGE
    )
    .execute(&pool)
    .await
    .map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Session creation failed")
    })?;

    tracing::info!(email = %p.email, user_id = %row.id, "user logged in");

    let mut headers = HeaderMap::new();
    let csrf_token = new_csrf_token();
    headers.append(SET_COOKIE, session_cookie(session_id));
    headers.append(SET_COOKIE, csrf_cookie(&csrf_token));
    headers.insert(
        axum::http::header::HeaderName::from_static("x-csrf-token"),
        HeaderValue::from_str(&csrf_token).expect("valid csrf token"),
    );
    Ok((
        StatusCode::OK,
        headers,
        Json(UserResponse {
            id: row.id,
            username: row.username,
            email: row.email,
            role: row.role,
            expires_at: now + SESSION_MAX_AGE,
        }),
    ))
}
