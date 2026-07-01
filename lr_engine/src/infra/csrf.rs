use axum::{
    body::Body,
    http::{HeaderMap, Method, Request, StatusCode},
    middleware::Next,
    response::Response,
};
use subtle::ConstantTimeEq;

/// Double-submit CSRF guard.
///
/// Safe (non-mutating) methods always pass. For mutating methods we only
/// enforce when the request carries a `session` cookie — i.e. an authenticated,
/// cookie-based browser request, the only thing a CSRF attack can abuse. The
/// pre-session auth endpoints (register/verify/login/password-reset) are signed
/// with Ed25519 envelopes instead, so they have no `csrf` cookie yet and are
/// intentionally exempt here.
///
/// When enforced, the `csrf` cookie value must match the `x-csrf-token` header
/// (both are issued together by login/verify/session in
/// `api::users::shared`).
pub async fn enforce_csrf(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    if is_safe(req.method()) {
        return Ok(next.run(req).await);
    }

    let headers = req.headers();

    // No session cookie -> unauthenticated request, nothing for CSRF to abuse.
    if cookie_value(headers, "session").is_none() {
        return Ok(next.run(req).await);
    }

    let cookie_token = cookie_value(headers, "csrf");
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
