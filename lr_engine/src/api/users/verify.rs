use axum::{
    Extension, Json,
    http::{HeaderMap, HeaderValue, StatusCode, header::SET_COOKIE},
};
use chrono::Utc;
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::shared::{
    E, SESSION_MAX_AGE, UserResponse, clear_legacy_domain_csrf_cookie, csrf_cookie,
    new_csrf_token, session_cookie, verify_code,
};
use crate::api::verified::Verified;

#[derive(Deserialize)]
struct VerifyInput {
    email: String,
    code: String,
}

pub async fn verify(
    Extension(pool): Extension<PgPool>,
    Verified(msg, _): Verified,
) -> Result<(StatusCode, HeaderMap, Json<UserResponse>), E> {
    let mut p: VerifyInput =
        serde_json::from_slice(&msg).map_err(|_| (StatusCode::BAD_REQUEST, "Invalid payload"))?;
    p.email = p.email.trim().to_ascii_lowercase();

    if p.code.len() != 6 || !p.code.chars().all(|c| c.is_ascii_digit()) {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Invalid code format"));
    }

    let now = Utc::now().timestamp();
    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Verification failed")
    })?;

    sqlx::query!(
        "DELETE FROM public.verification_codes WHERE expires_at <= $1",
        now
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Verification failed")
    })?;

    let vc = sqlx::query!(
        "SELECT username, password_hash, code, expires_at, access_token
         FROM public.verification_codes WHERE email = $1 FOR UPDATE",
        p.email
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Verification failed")
    })?
    // same message as a wrong code — "no pending verification" would tell a
    // prober that register silently skipped an already-registered email
    .ok_or((
        StatusCode::UNPROCESSABLE_ENTITY,
        "Incorrect verification code",
    ))?;

    if vc.expires_at <= now {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "Verification code has expired",
        ));
    }

    let attempts: i32 = sqlx::query_scalar::<_, i32>(
        "SELECT failed_attempts FROM public.verification_codes WHERE email = $1",
    )
    .bind(&p.email)
    .fetch_one(&mut *tx)
    .await
    .unwrap_or(0);

    if attempts >= 5 {
        sqlx::query!(
            "DELETE FROM public.verification_codes WHERE email = $1",
            p.email
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!("DB: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Verification failed")
        })?;
        tx.commit().await.ok();
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "Too many incorrect attempts. Please request a new code.",
        ));
    }

    if !verify_code(&p.code, &vc.code) {
        let _ = sqlx::query(
            "UPDATE public.verification_codes SET failed_attempts = failed_attempts + 1 WHERE email = $1"
        )
        .bind(&p.email)
        .execute(&mut *tx)
        .await;
        tx.commit().await.ok();
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "Incorrect verification code",
        ));
    }

    // Re-check the access token now rather than trusting the register-time
    // validation: up to 10 minutes pass between the two, and a token revoked
    // (or expired, or redeemed by someone else) in that window must not still
    // buy the elevated role. Falls back to Pending, never fails the signup.
    let token_still_valid = match vc.access_token.as_deref().filter(|t| !t.is_empty()) {
        None => false,
        Some(token) => sqlx::query!(
            "SELECT token FROM public.access_tokens
              WHERE token = $1
                AND revoked_at IS NULL
                AND redeemed_at IS NULL
                AND (expires_at IS NULL OR expires_at > $2)
              FOR UPDATE",
            token,
            now
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!("DB: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Verification failed")
        })?
        .is_some(),
    };
    let role = if token_still_valid { "User" } else { "Pending" };

    let user_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO public.users (id, username, email, password_hash, role, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $6)",
        user_id, vc.username, p.email, vc.password_hash, role, now
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Account creation failed")
    })?;

    if token_still_valid && let Some(ref token) = vc.access_token {
        sqlx::query!(
            "UPDATE public.access_tokens SET redeemed_by = $1, redeemed_at = $2 WHERE token = $3",
            user_id,
            now,
            token
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!("DB: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Verification failed")
        })?;
    }

    sqlx::query!(
        "DELETE FROM public.verification_codes WHERE email = $1 OR expires_at <= $2",
        p.email,
        now
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Verification failed")
    })?;

    sqlx::query!(
        "DELETE FROM public.sessions WHERE user_id = $1 OR expires_at <= $2",
        user_id,
        now
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Session cleanup failed")
    })?;

    let session_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO public.sessions (id, user_id, created_at, expires_at) VALUES ($1, $2, $3, $4)",
        session_id,
        user_id,
        now,
        now + SESSION_MAX_AGE
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Session creation failed")
    })?;

    tx.commit().await.map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Account creation failed")
    })?;

    tracing::info!(email = %p.email, %user_id, "user registered");

    let mut headers = HeaderMap::new();
    let csrf_token = new_csrf_token();
    headers.append(SET_COOKIE, session_cookie(session_id));
    headers.append(SET_COOKIE, csrf_cookie(&csrf_token));
    if let Some(clear) = clear_legacy_domain_csrf_cookie() {
        headers.append(SET_COOKIE, clear);
    }
    headers.insert(
        axum::http::header::HeaderName::from_static("x-csrf-token"),
        HeaderValue::from_str(&csrf_token).expect("valid csrf token"),
    );
    Ok((
        StatusCode::CREATED,
        headers,
        Json(UserResponse {
            id: user_id,
            username: vc.username,
            email: p.email,
            role: role.into(),
            created_at: now, // the row was just inserted with created_at = now above
            expires_at: now + SESSION_MAX_AGE,
        }),
    ))
}
