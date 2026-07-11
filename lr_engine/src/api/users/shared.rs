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

/// Global cap on concurrent Argon2 work. Each hash/verify pins ~64MB
/// (m=65536), and the rate limiters only bound request *rate*, not how many
/// hashes are in flight at once — a synchronized burst across many IPs could
/// stack enough 64MB allocations to OOM the instance. Excess requests queue
/// here for the ~100–300ms a hash takes instead. 4 permits ≈ 256MB worst
/// case; raise if the instance has memory to spare.
static HASH_PERMITS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);

/// Acquire a slot for one Argon2 hash/verify. Drop the permit as soon as the
/// hashing is done — never hold it across DB calls, so a task waiting here
/// can never be waited on by a permit holder (no deadlock).
pub async fn hash_permit() -> tokio::sync::SemaphorePermit<'static> {
    HASH_PERMITS
        .acquire()
        .await
        .expect("hash semaphore is never closed")
}

pub const SESSION_MAX_AGE: i64 = 24 * 3600; // 1 day
pub const MAX_PASSWORD_LEN: usize = 128;

#[derive(Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub role: String,
    /// Unix seconds — for display only ("Member since"), not used in any auth decision.
    pub created_at: i64,
    pub expires_at: i64,
}

#[derive(Serialize)]
pub struct MessageResponse {
    pub message: &'static str,
}

pub fn extract_session_id(headers: &HeaderMap) -> Option<Uuid> {
    extract_cookie_value(headers, session_cookie_name())?
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

/// Secure is the *default*; COOKIE_INSECURE_DEV=1 opts out for plain-http
/// local/LAN dev, where browsers refuse Secure (and __Host-) cookies. This
/// replaces the old opt-in COOKIE_SECURE, which failed open when forgotten
/// in a production environment.
fn insecure_dev() -> bool {
    static DEV: OnceLock<bool> = OnceLock::new();
    *DEV.get_or_init(|| std::env::var("COOKIE_INSECURE_DEV").is_ok())
}

/// __Host- prefixed in secure mode: the browser itself then rejects the
/// cookie unless it is Secure, host-only (no Domain), and Path=/ — subdomain
/// cookie-tossing and downgrade tricks stop at the cookie jar. The prefix is
/// illegal without Secure, so plain-http dev keeps the bare names.
pub fn session_cookie_name() -> &'static str {
    if insecure_dev() { "session" } else { "__Host-session" }
}

pub fn csrf_cookie_name() -> &'static str {
    if insecure_dev() { "csrf" } else { "__Host-csrf" }
}

/// The app and API share a registrable domain (COOKIE_DOMAIN in prod), so
/// SameSite=Lax works in both environments and keeps the browser's built-in
/// cross-site protection as a second line of defense next to the CSRF check —
/// None would hand that entirely to the double-submit guard.
fn cookie_security_attrs() -> &'static str {
    if insecure_dev() {
        "SameSite=Lax"
    } else {
        "SameSite=Lax; Secure"
    }
}

pub fn session_cookie(session_id: Uuid) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "{}={session_id}; HttpOnly; {}; Path=/; Max-Age={SESSION_MAX_AGE}",
        session_cookie_name(),
        cookie_security_attrs()
    ))
    .expect("valid cookie value")
}

pub fn new_csrf_token() -> String {
    Uuid::new_v4().to_string()
}

/// Host-only (no Domain attribute — a Domain cookie is exposed to every
/// subdomain, e.g. the CDN) and HttpOnly: the frontend never reads this
/// cookie, it takes the token from the x-csrf-token response header, so
/// there's no reason to leave it readable to scripts.
pub fn csrf_cookie(token: &str) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "{}={token}; HttpOnly; {}; Path=/; Max-Age={SESSION_MAX_AGE}",
        csrf_cookie_name(),
        cookie_security_attrs(),
    ))
    .expect("valid cookie value")
}

pub fn clear_session_cookie() -> HeaderValue {
    HeaderValue::from_str(&format!(
        "{}=; HttpOnly; {}; Path=/; Max-Age=0",
        session_cookie_name(),
        cookie_security_attrs()
    ))
    .expect("valid cookie value")
}

pub fn clear_csrf_cookie() -> HeaderValue {
    HeaderValue::from_str(&format!(
        "{}=; HttpOnly; {}; Path=/; Max-Age=0",
        csrf_cookie_name(),
        cookie_security_attrs(),
    ))
    .expect("valid cookie value")
}

/// Earlier builds issued the csrf cookie with `Domain=<COOKIE_DOMAIN>`. Those
/// cookies coexist with the new host-only one under the same name, and which
/// of the two the browser sends first is unspecified — so wherever a fresh
/// csrf cookie is set, the legacy domain-scoped one must be explicitly
/// deleted. Returns None when no COOKIE_DOMAIN is configured (nothing to
/// clean up). Delete this once prod sessions from before the change have aged
/// out (session lifetime is 24h).
pub fn clear_legacy_domain_csrf_cookie() -> Option<HeaderValue> {
    let domain = std::env::var("COOKIE_DOMAIN").ok()?;
    let domain = domain.trim();
    if domain.is_empty() {
        return None;
    }
    HeaderValue::from_str(&format!(
        "csrf=; {}; Domain={domain}; Path=/; Max-Age=0",
        cookie_security_attrs(),
    ))
    .ok()
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

/// Collapse whatever the client sent (mirroring `AccentProvider`'s `Accent`
/// type) to one of the two themes the mailer actually has palettes for.
/// Anything unrecognized — including absent, an older client, or tampering —
/// falls back to "blue" rather than erroring, since a wrong-but-valid email
/// theme is cosmetic, not a security boundary.
pub fn normalize_mail_theme(theme: Option<&str>) -> &'static str {
    match theme {
        Some("green") => "green",
        _ => "blue",
    }
}
