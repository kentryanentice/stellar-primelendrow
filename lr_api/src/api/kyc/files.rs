use axum::{
    Extension,
    body::Body,
    extract::{Path, Query},
    http::{StatusCode, header},
    response::Response,
};
use serde::Deserialize;

use crate::api::users::shared::E;
use crate::infra::storage::MongoStorage;

#[derive(Deserialize)]
pub struct SignedQuery {
    exp: i64,
    sig: String,
}

/// GET /kyc/files/{path}?exp=..&sig=..
///
/// Serves a private KYC document to whoever holds a fresh signed URL — the
/// same access model as the Supabase signed URLs this replaces: the HMAC in
/// the query string *is* the authorization, minted only for admins by the
/// detail endpoint and dead within minutes. Constant-time verification, and
/// a wrong/expired signature answers before any database work.
pub async fn file(
    Extension(storage): Extension<MongoStorage>,
    Path(path): Path<String>,
    Query(q): Query<SignedQuery>,
) -> Result<Response, E> {
    if !storage.verify_signature(&path, q.exp, &q.sig) {
        return Err((StatusCode::FORBIDDEN, "Invalid or expired link"));
    }

    let (bytes, content_type) = storage
        .fetch(&path)
        .await
        .map_err(|e| {
            tracing::error!("KYC file fetch: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Unable to load document")
        })?
        .ok_or((StatusCode::NOT_FOUND, "Document not found"))?;

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::CONTENT_DISPOSITION, "inline")
        .body(Body::from(bytes))
        .map_err(|e| {
            tracing::error!("KYC file response: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Unable to load document")
        })
}
