use axum::Extension;
use axum::http::{HeaderMap, StatusCode, header::SET_COOKIE};
use mongodb::{Database, bson::doc};

use super::shared::{
    clear_csrf_cookie, clear_legacy_domain_csrf_cookie, clear_session_cookie, extract_session_id,
};

pub async fn logout(
    Extension(db): Extension<Database>,
    headers: HeaderMap,
) -> (StatusCode, HeaderMap) {
    if let Some(sid) = extract_session_id(&headers) {
        let _ = db
            .collection::<mongodb::bson::Document>("sessions")
            .delete_one(doc! { "_id": sid.to_string() })
            .await;
    }
    let mut h = HeaderMap::new();
    h.append(SET_COOKIE, clear_session_cookie());
    h.append(SET_COOKIE, clear_csrf_cookie());
    if let Some(clear) = clear_legacy_domain_csrf_cookie() {
        h.append(SET_COOKIE, clear);
    }
    (StatusCode::NO_CONTENT, h)
}
