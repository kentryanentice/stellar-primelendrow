use axum::{
    Extension, Json,
    http::{HeaderMap, HeaderValue, StatusCode, header::SET_COOKIE},
};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use super::shared::{
    UserResponse, clear_csrf_cookie, clear_legacy_domain_csrf_cookie, clear_session_cookie,
    csrf_cookie, csrf_cookie_name, extract_cookie_value, extract_session_id, new_csrf_token,
};

/// A 401 from here means the browser's cookies no longer match a live
/// session row; clear both auth cookies so the stale pair doesn't ride along
/// (tripping the CSRF guard into 403s) for the rest of its Max-Age.
fn unauthenticated(msg: &'static str) -> (StatusCode, HeaderMap, &'static str) {
    let mut headers = HeaderMap::new();
    headers.append(SET_COOKIE, clear_session_cookie());
    headers.append(SET_COOKIE, clear_csrf_cookie());
    (StatusCode::UNAUTHORIZED, headers, msg)
}

pub async fn session_handler(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Json<UserResponse>), (StatusCode, HeaderMap, &'static str)> {
    let now = Utc::now().timestamp();
    let sid = extract_session_id(&headers).ok_or_else(|| unauthenticated("Not authenticated"))?;

    let row = sqlx::query!(
        "SELECT u.id AS \"id: Uuid\", u.username, u.email, u.role, s.expires_at
         FROM public.sessions s
         JOIN public.users u ON u.id = s.user_id
         WHERE s.id = $1 AND s.expires_at > $2",
        sid,
        now
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, HeaderMap::new(), "DB error")
    })?
    .ok_or_else(|| unauthenticated("Session expired or not found"))?;

    // separate runtime-checked query (not folded into the query! above) so
    // this needs no offline sqlx-data regeneration — display-only field
    let created_at: i64 = sqlx::query_scalar("SELECT created_at FROM public.users WHERE id = $1")
        .bind(row.id)
        .fetch_one(&pool)
        .await
        .map_err(|e| {
            tracing::error!("DB user created_at: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, HeaderMap::new(), "DB error")
        })?;

    let mut response_headers = HeaderMap::new();
    // Reuse the caller's existing csrf token rather than minting a fresh one
    // per call: rotating here silently invalidated the in-memory token of
    // every other open tab, 403ing its next mutation. Login/verify still
    // rotate — new session, new token.
    let csrf_token = match extract_cookie_value(&headers, csrf_cookie_name()) {
        Some(existing) => existing,
        None => {
            let fresh = new_csrf_token();
            response_headers.append(SET_COOKIE, csrf_cookie(&fresh));
            fresh
        }
    };
    if let Some(clear) = clear_legacy_domain_csrf_cookie() {
        response_headers.append(SET_COOKIE, clear);
    }
    response_headers.insert(
        axum::http::header::HeaderName::from_static("x-csrf-token"),
        HeaderValue::from_str(&csrf_token).expect("valid csrf token"),
    );

    Ok((
        response_headers,
        Json(UserResponse {
            id: row.id,
            username: row.username,
            email: row.email,
            role: row.role,
            created_at,
            expires_at: row.expires_at,
        }),
    ))
}
