use axum::http::{HeaderMap, HeaderValue, StatusCode};
use chrono::Utc;
use hmac::{Hmac, Mac};
use serde::Serialize;
use sha2::Sha256;
use sqlx::PgPool;
use std::sync::OnceLock;
use subtle::ConstantTimeEq;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

pub type E = (StatusCode, &'static str);

pub const SESSION_MAX_AGE: i64 = 24 * 3600; // 1 day
pub const MAX_PASSWORD_LEN: usize = 128;

#[derive(Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub role: String,
    pub expires_at: i64,
}

#[derive(Serialize)]
pub struct MessageResponse {
    pub message: &'static str,
}

pub fn extract_session_id(headers: &HeaderMap) -> Option<Uuid> {
    extract_cookie_value(headers, "session")?
        .parse::<Uuid>()
        .ok()
}

pub async fn require_admin(pool: &PgPool, headers: &HeaderMap) -> Result<Uuid, E> {
    let now = Utc::now().timestamp();
    let sid = extract_session_id(headers).ok_or((StatusCode::UNAUTHORIZED, "Not authenticated"))?;

    let admin_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT u.id
           FROM public.sessions s
           JOIN public.users u ON u.id = s.user_id
          WHERE s.id = $1
            AND s.expires_at > $2
            AND u.role = 'Admin'",
    )
    .bind(sid)
    .bind(now)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("DB admin session: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "DB error")
    })?;

    admin_id.ok_or((StatusCode::FORBIDDEN, "Admin access required"))
}

pub async fn require_user(pool: &PgPool, headers: &HeaderMap) -> Result<Uuid, E> {
    let now = Utc::now().timestamp();
    let sid = extract_session_id(headers).ok_or((StatusCode::UNAUTHORIZED, "Not authenticated"))?;

    let user_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT u.id
           FROM public.sessions s
           JOIN public.users u ON u.id = s.user_id
          WHERE s.id = $1 AND s.expires_at > $2",
    )
    .bind(sid)
    .bind(now)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("DB user session: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "DB error")
    })?;

    user_id.ok_or((StatusCode::UNAUTHORIZED, "Not authenticated"))
}

pub async fn require_verified_user(pool: &PgPool, headers: &HeaderMap) -> Result<Uuid, E> {
    let now = Utc::now().timestamp();
    let sid = extract_session_id(headers).ok_or((StatusCode::UNAUTHORIZED, "Not authenticated"))?;

    let user_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT u.id
           FROM public.sessions s
           JOIN public.users u ON u.id = s.user_id
          WHERE s.id = $1
            AND s.expires_at > $2
            AND u.role IN ('User', 'Admin')",
    )
    .bind(sid)
    .bind(now)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("DB verified user session: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "DB error")
    })?;

    user_id.ok_or((StatusCode::FORBIDDEN, "Account approval required"))
}

pub fn extract_cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let cookie_str = headers.get("cookie")?.to_str().ok()?;
    let prefix = format!("{name}=");
    for part in cookie_str.split(';') {
        if let Some(val) = part.trim().strip_prefix(&prefix) {
            return Some(val.trim().to_string());
        }
    }
    None
}

fn cookie_security_attrs() -> &'static str {
    if std::env::var("COOKIE_SECURE").is_ok() {
        "SameSite=None; Secure"
    } else {
        "SameSite=Lax"
    }
}

fn cookie_domain_attr() -> String {
    match std::env::var("COOKIE_DOMAIN") {
        Ok(domain) if !domain.trim().is_empty() => format!("; Domain={}", domain.trim()),
        _ => String::new(),
    }
}

pub fn session_cookie(session_id: Uuid) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "session={session_id}; HttpOnly; {}; Path=/; Max-Age={SESSION_MAX_AGE}",
        cookie_security_attrs()
    ))
    .expect("valid cookie value")
}

pub fn new_csrf_token() -> String {
    Uuid::new_v4().to_string()
}

pub fn csrf_cookie(token: &str) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "csrf={token}; {}{}; Path=/; Max-Age={SESSION_MAX_AGE}",
        cookie_security_attrs(),
        cookie_domain_attr(),
    ))
    .expect("valid cookie value")
}

pub fn clear_session_cookie() -> HeaderValue {
    HeaderValue::from_str(&format!(
        "session=; HttpOnly; {}; Path=/; Max-Age=0",
        cookie_security_attrs()
    ))
    .expect("valid cookie value")
}

pub fn clear_csrf_cookie() -> HeaderValue {
    HeaderValue::from_str(&format!(
        "csrf=; {}{}; Path=/; Max-Age=0",
        cookie_security_attrs(),
        cookie_domain_attr(),
    ))
    .expect("valid cookie value")
}

pub fn is_valid_email(email: &str) -> bool {
    let parts: Vec<&str> = email.splitn(2, '@').collect();
    parts.len() == 2 && !parts[0].is_empty() && parts[1].contains('.') && parts[1].len() > 2
}

/// Server-side pepper used to HMAC one-time codes before storing them.
/// Read once from `OTP_HASH_SECRET`. An empty key still hashes (so we never
/// store plaintext), but a real secret is what makes the 6-digit code space
/// infeasible to brute-force if the database is ever exposed.
fn code_secret() -> &'static [u8] {
    static SECRET: OnceLock<Vec<u8>> = OnceLock::new();
    SECRET.get_or_init(|| {
        std::env::var("OTP_HASH_SECRET")
            .unwrap_or_default()
            .into_bytes()
    })
}

/// HMAC-SHA256 of a one-time code, hex-encoded, for storage at rest.
pub fn hash_code(code: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(code_secret()).expect("HMAC accepts any key length");
    mac.update(code.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Constant-time check of a presented code against a stored hash.
pub fn verify_code(code: &str, stored_hash: &str) -> bool {
    hash_code(code)
        .as_bytes()
        .ct_eq(stored_hash.as_bytes())
        .into()
}

pub fn is_strong_password(pw: &str) -> bool {
    pw.len() >= 8
        && pw.len() <= MAX_PASSWORD_LEN
        && pw.chars().any(|c| c.is_uppercase())
        && pw.chars().any(|c| !c.is_alphanumeric())
}
