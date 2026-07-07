use axum::{Extension, Json, http::HeaderMap};
use mongodb::{Database, bson::doc};
use serde::{Deserialize, Serialize};

use crate::api::users::shared::{E, require_user};

#[derive(Serialize)]
pub struct KycStatusResponse {
    /// "none" | "pending" | "approved" | "rejected"
    pub status: String,
    pub submitted_at: Option<i64>,
    pub reviewed_at: Option<i64>,
    /// Only ever present on the owner's own rejected submission.
    pub rejection_reason: Option<String>,
}

#[derive(Deserialize)]
struct StatusDoc {
    status: String,
    created_at: i64,
    #[serde(default)]
    reviewed_at: Option<i64>,
    #[serde(default)]
    rejection_reason: Option<String>,
}

/// The owner's view of their latest submission. Deliberately returns no PII —
/// the client already knows what it typed, and echoing decrypted fields would
/// turn a stolen session cookie into a document leak.
pub async fn status(
    Extension(db): Extension<Database>,
    headers: HeaderMap,
) -> Result<Json<KycStatusResponse>, E> {
    let user_id = require_user(&db, &headers).await?;

    let row = db
        .collection::<StatusDoc>("kyc_submissions")
        .find_one(doc! { "user_id": user_id.to_string() })
        .sort(doc! { "created_at": -1 })
        .await
        .map_err(|e| {
            tracing::error!("DB kyc status: {e}");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Unable to load status",
            )
        })?;

    Ok(Json(match row {
        None => KycStatusResponse {
            status: "none".into(),
            submitted_at: None,
            reviewed_at: None,
            rejection_reason: None,
        },
        Some(doc) => {
            let rejection_reason = (doc.status == "rejected")
                .then_some(doc.rejection_reason)
                .flatten();
            KycStatusResponse {
                status: doc.status,
                submitted_at: Some(doc.created_at),
                reviewed_at: doc.reviewed_at,
                rejection_reason,
            }
        }
    }))
}
