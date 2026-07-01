use axum::Extension;
use axum::http::{HeaderMap, StatusCode, header::SET_COOKIE};
use sqlx::PgPool;

use super::shared::{clear_csrf_cookie, clear_session_cookie, extract_session_id};

pub async fn logout(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
) -> (StatusCode, HeaderMap) {
    if let Some(sid) = extract_session_id(&headers) {
        let _ = sqlx::query!("DELETE FROM public.sessions WHERE id = $1", sid)
            .execute(&pool)
            .await;
    }
    let mut h = HeaderMap::new();
    h.append(SET_COOKIE, clear_session_cookie());
    h.append(SET_COOKIE, clear_csrf_cookie());
    (StatusCode::NO_CONTENT, h)
}
