use axum::{
    body::Body,
    http::{HeaderMap, Method, Request, StatusCode},
    middleware::Next,
    response::Response,
};
use subtle::ConstantTimeEq;

use crate::api::users::shared::{csrf_cookie_name, session_cookie_name};

/// Pre-session auth endpoints whose CSRF protection is the Ed25519 envelope
/// (nonce + ingress expiry) rather than the double-submit cookie.
const ENVELOPE_SIGNED_PATHS: &[&str] = &[
    "/auth/register",
    "/auth/verify",
    "/auth/login",
    "/auth/password-reset/request",
    "/auth/password-reset/confirm",
];

/// Double-submit CSRF guard.
///
/// Safe (non-mutating) methods always pass. The pre-session auth endpoints
/// (`ENVELOPE_SIGNED_PATHS`) are exempt by path: they must stay reachable
/// while the browser carries a stale `session` cookie whose DB row is already
/// gone — gating the exemption on cookie absence let a dead cookie 403 the
/// user out of logging back in for its full Max-Age.
///
/// For the remaining mutating methods we only enforce when the request
/// carries a `session` cookie — i.e. an authenticated, cookie-based browser
/// request, the only thing a CSRF attack can abuse.
///
/// When enforced, the `csrf` cookie value must match the `x-csrf-token` header
/// (both are issued together by login/verify/session in
/// `api::users::shared`).
pub async fn enforce_csrf(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    if is_safe(req.method()) || ENVELOPE_SIGNED_PATHS.contains(&req.uri().path()) {
        return Ok(next.run(req).await);
    }

    let headers = req.headers();

    // No session cookie -> unauthenticated request, nothing for CSRF to abuse.
    if cookie_value(headers, session_cookie_name()).is_none() {
        return Ok(next.run(req).await);
    }

    let cookie_token = cookie_value(headers, csrf_cookie_name());
    let header_token = headers
        .get("x-csrf-token")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    match (cookie_token, header_token) {
        (Some(c), Some(h)) if c.as_bytes().ct_eq(h.as_bytes()).into() => Ok(next.run(req).await),
        _ => Err(StatusCode::FORBIDDEN),
    }
}

fn is_safe(method: &Method) -> bool {
    matches!(
        *method,
        Method::GET | Method::HEAD | Method::OPTIONS | Method::TRACE
    )
}

fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let cookies = headers.get("cookie")?.to_str().ok()?;
    let prefix = format!("{name}=");
    for part in cookies.split(';') {
        if let Some(val) = part.trim().strip_prefix(&prefix) {
            return Some(val.trim().to_string());
        }
    }
    None
}
