use axum::{
    Extension, Json,
    http::{HeaderMap, HeaderValue, StatusCode, header::SET_COOKIE},
};
use chrono::Utc;
use mongodb::{Database, bson::doc};
use serde::Deserialize;
use uuid::Uuid;

use super::shared::{
    UserResponse, clear_csrf_cookie, clear_legacy_domain_csrf_cookie, clear_session_cookie,
    csrf_cookie, csrf_cookie_name, extract_cookie_value, extract_session_id, new_csrf_token,
};

#[derive(Deserialize)]
struct SessionDoc {
    user_id: String,
    expires_at: i64,
}

#[derive(Deserialize)]
struct UserDoc {
    #[serde(rename = "_id")]
    id: String,
    username: String,
    email: String,
    role: String,
}

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
    Extension(db): Extension<Database>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Json<UserResponse>), (StatusCode, HeaderMap, &'static str)> {
    let now = Utc::now().timestamp();
    let sid = extract_session_id(&headers).ok_or_else(|| unauthenticated("Not authenticated"))?;

    let db_err = |e: mongodb::error::Error| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, HeaderMap::new(), "DB error")
    };

    let session = db
        .collection::<SessionDoc>("sessions")
        .find_one(doc! { "_id": sid.to_string(), "expires_at": { "$gt": now } })
        .await
        .map_err(db_err)?
        .ok_or_else(|| unauthenticated("Session expired or not found"))?;

    let user = db
        .collection::<UserDoc>("users")
        .find_one(doc! { "_id": &session.user_id })
        .await
        .map_err(db_err)?
        .ok_or_else(|| unauthenticated("Session expired or not found"))?;

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
            id: Uuid::parse_str(&user.id).unwrap_or(Uuid::nil()),
            username: user.username,
            email: user.email,
            role: user.role,
            expires_at: session.expires_at,
        }),
    ))
}
