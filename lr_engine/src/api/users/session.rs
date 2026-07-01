use axum::{
    Extension, Json,
    http::{HeaderMap, HeaderValue, StatusCode, header::SET_COOKIE},
};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use super::shared::{E, UserResponse, csrf_cookie, extract_session_id, new_csrf_token};

pub async fn session_handler(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Json<UserResponse>), E> {
    let now = Utc::now().timestamp();
    let sid =
        extract_session_id(&headers).ok_or((StatusCode::UNAUTHORIZED, "Not authenticated"))?;

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
        (StatusCode::INTERNAL_SERVER_ERROR, "DB error")
    })?
    .ok_or((StatusCode::UNAUTHORIZED, "Session expired or not found"))?;

    let mut response_headers = HeaderMap::new();
    let csrf_token = new_csrf_token();
    response_headers.append(SET_COOKIE, csrf_cookie(&csrf_token));
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
            expires_at: row.expires_at,
        }),
    ))
}
