use axum::{
    Extension, Json,
    extract::Path,
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use mongodb::{
    Database,
    bson::{Binary, doc},
    options::ReturnDocument,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::shared::{MAX_REASON_LEN, audit};
use crate::api::users::shared::{E, MessageResponse, require_admin};
use crate::infra::{crypto, storage::MongoStorage};

/// How long an admin's signed image URL stays valid. Long enough to review,
/// short enough that a leaked URL from a screen-share is soon worthless.
const SIGNED_URL_TTL_SECS: u32 = 5 * 60;

// ---- pending list: metadata only, no PII ----
//
// Listing is cheap to poll, so it must not decrypt anything; PII access is a
// deliberate, per-submission, audited act via `detail` below.

#[derive(Serialize)]
pub struct PendingItem {
    pub id: String,
    pub user_id: String,
    pub id_type: String,
    pub face_match_score: Option<i16>,
    pub liveness_passed: bool,
    pub created_at: i64,
}

#[derive(Deserialize)]
struct PendingDoc {
    #[serde(rename = "_id")]
    id: String,
    user_id: String,
    id_type: String,
    #[serde(default)]
    face_match_score: Option<i32>,
    #[serde(default)]
    liveness_passed: bool,
    created_at: i64,
}

pub async fn pending(
    Extension(db): Extension<Database>,
    headers: HeaderMap,
) -> Result<Json<Vec<PendingItem>>, E> {
    require_admin(&db, &headers).await?;

    let db_err = |e: mongodb::error::Error| {
        tracing::error!("DB kyc pending: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Unable to load queue")
    };

    let mut cursor = db
        .collection::<PendingDoc>("kyc_submissions")
        .find(doc! { "status": "pending" })
        .sort(doc! { "created_at": 1 })
        .limit(100)
        .await
        .map_err(db_err)?;

    let mut items = Vec::new();
    while cursor.advance().await.map_err(db_err)? {
        let d = cursor.deserialize_current().map_err(|e| {
            tracing::error!("DB kyc pending decode: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Unable to load queue")
        })?;
        items.push(PendingItem {
            id: d.id,
            user_id: d.user_id,
            id_type: d.id_type,
            face_match_score: d.face_match_score.and_then(|v| i16::try_from(v).ok()),
            liveness_passed: d.liveness_passed,
            created_at: d.created_at,
        });
    }

    Ok(Json(items))
}

// ---- detail: decrypted PII + short-lived signed image URLs, audited ----

#[derive(Serialize)]
pub struct SubmissionDetail {
    pub id: String,
    pub user_id: String,
    pub status: String,
    pub id_type: String,
    pub first_name: String,
    pub middle_name: Option<String>,
    pub last_name: String,
    pub dob: String,
    pub id_number: String,
    pub wallet_address: Option<String>,
    pub face_match_score: Option<i16>,
    pub liveness_passed: bool,
    /// Signed URLs, valid for a few minutes; None once purged.
    pub id_image_url: Option<String>,
    pub selfie_image_url: Option<String>,
    pub rejection_reason: Option<String>,
    pub created_at: i64,
    pub reviewed_at: Option<i64>,
}

#[derive(Deserialize)]
struct DetailDoc {
    user_id: String,
    status: String,
    id_type: String,
    first_name_enc: Binary,
    #[serde(default)]
    middle_name_enc: Option<Binary>,
    last_name_enc: Binary,
    dob_enc: Binary,
    id_number_enc: Binary,
    #[serde(default)]
    wallet_address: Option<String>,
    #[serde(default)]
    face_match_score: Option<i32>,
    #[serde(default)]
    liveness_passed: bool,
    #[serde(default)]
    id_image_path: Option<String>,
    #[serde(default)]
    selfie_image_path: Option<String>,
    #[serde(default)]
    rejection_reason: Option<String>,
    created_at: i64,
    #[serde(default)]
    reviewed_at: Option<i64>,
}

pub async fn detail(
    Extension(db): Extension<Database>,
    Extension(storage): Extension<MongoStorage>,
    headers: HeaderMap,
    Path(submission_id): Path<Uuid>,
) -> Result<Json<SubmissionDetail>, E> {
    let admin_id = require_admin(&db, &headers).await?;

    let row = db
        .collection::<DetailDoc>("kyc_submissions")
        .find_one(doc! { "_id": submission_id.to_string() })
        .await
        .map_err(|e| {
            tracing::error!("DB kyc detail: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Unable to load submission")
        })?
        .ok_or((StatusCode::NOT_FOUND, "Submission not found"))?;

    let user_id = Uuid::parse_str(&row.user_id).unwrap_or(Uuid::nil());

    let open = |sealed: &[u8]| {
        crypto::open(sealed).map_err(|e| {
            tracing::error!(%submission_id, "KYC decrypt failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Unable to load submission")
        })
    };
    let first_name = open(&row.first_name_enc.bytes)?;
    let middle_name = row
        .middle_name_enc
        .as_ref()
        .map(|b| open(&b.bytes))
        .transpose()?;
    let last_name = open(&row.last_name_enc.bytes)?;
    let dob = open(&row.dob_enc.bytes)?;
    let id_number = open(&row.id_number_enc.bytes)?;

    // signed-URL failures degrade to no image rather than failing the review
    let sign = |path: Option<String>| async {
        match path {
            None => None,
            Some(p) => match storage.signed_url(&p, SIGNED_URL_TTL_SECS).await {
                Ok(url) => Some(url),
                Err(e) => {
                    tracing::error!(%submission_id, "KYC sign url failed: {e}");
                    None
                }
            },
        }
    };
    let id_image_url = sign(row.id_image_path).await;
    let selfie_image_url = sign(row.selfie_image_path).await;

    // every decrypt of a person's documents leaves a trace of who looked
    audit(
        &db,
        submission_id,
        user_id,
        Some(admin_id),
        "viewed",
        Some("decrypted PII and signed image URLs"),
    )
    .await;

    Ok(Json(SubmissionDetail {
        id: submission_id.to_string(),
        user_id: row.user_id,
        status: row.status,
        id_type: row.id_type,
        first_name,
        middle_name,
        last_name,
        dob,
        id_number,
        wallet_address: row.wallet_address,
        face_match_score: row.face_match_score.and_then(|v| i16::try_from(v).ok()),
        liveness_passed: row.liveness_passed,
        id_image_url,
        selfie_image_url,
        rejection_reason: row.rejection_reason,
        created_at: row.created_at,
        reviewed_at: row.reviewed_at,
    }))
}

// ---- review decision ----

#[derive(Deserialize)]
pub struct ReviewInput {
    submission_id: Uuid,
    /// "approve" | "reject"
    decision: String,
    #[serde(default)]
    reason: Option<String>,
}

pub async fn review(
    Extension(db): Extension<Database>,
    Extension(storage): Extension<MongoStorage>,
    headers: HeaderMap,
    Json(p): Json<ReviewInput>,
) -> Result<Json<MessageResponse>, E> {
    let admin_id = require_admin(&db, &headers).await?;

    let approve = match p.decision.as_str() {
        "approve" => true,
        "reject" => false,
        _ => return Err((StatusCode::UNPROCESSABLE_ENTITY, "Unknown decision")),
    };
    let reason = p.reason.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if !approve && reason.is_none() {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "A rejection needs a reason"));
    }
    if reason.is_some_and(|r| r.len() > MAX_REASON_LEN) {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Reason too long"));
    }

    let db_err = |e: mongodb::error::Error| {
        tracing::error!("DB kyc review: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Review failed")
    };

    let now = Utc::now().timestamp();
    let status = if approve { "approved" } else { "rejected" };

    let submissions = db.collection::<mongodb::bson::Document>("kyc_submissions");

    // Decision and role promotion happen together or not at all.
    let mut session = db.client().start_session().await.map_err(db_err)?;
    session.start_transaction().await.map_err(db_err)?;

    // status guard in the filter makes the decision idempotent and
    // race-safe: two admins deciding at once — exactly one wins
    let row = submissions
        .find_one_and_update(
            doc! { "_id": p.submission_id.to_string(), "status": "pending" },
            doc! { "$set": {
                "status": status,
                "reviewed_by": admin_id.to_string(),
                "reviewed_at": now,
                "rejection_reason": if approve { None } else { reason },
                "updated_at": now,
            } },
        )
        .return_document(ReturnDocument::Before)
        .session(&mut session)
        .await
        .map_err(db_err)?;

    let Some(row) = row else {
        let _ = session.abort_transaction().await;
        return Err((StatusCode::NOT_FOUND, "No pending submission with that id"));
    };

    let user_id_str = row.get_str("user_id").unwrap_or_default().to_string();
    let user_id = Uuid::parse_str(&user_id_str).unwrap_or(Uuid::nil());
    let id_image_path = row.get_str("id_image_path").ok().map(str::to_string);
    let selfie_image_path = row.get_str("selfie_image_path").ok().map(str::to_string);

    if approve {
        // verified identity unlocks the account: Pending -> User. Admins keep
        // their role; the guard also stops a demotion if roles ever grow.
        db.collection::<mongodb::bson::Document>("users")
            .update_one(
                doc! { "_id": &user_id_str, "role": "Pending" },
                doc! { "$set": { "role": "User", "updated_at": now } },
            )
            .session(&mut session)
            .await
            .map_err(|e| {
                tracing::error!("DB kyc promote: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Review failed")
            })?;
    } else {
        // data minimization: a rejected applicant's documents have no reason
        // to stay on file — clear the paths now, delete the objects after
        // commit (storage delete is best-effort/idempotent)
        submissions
            .update_one(
                doc! { "_id": p.submission_id.to_string() },
                doc! { "$set": { "id_image_path": null, "selfie_image_path": null } },
            )
            .session(&mut session)
            .await
            .map_err(|e| {
                tracing::error!("DB kyc purge paths: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Review failed")
            })?;
    }

    session.commit_transaction().await.map_err(db_err)?;

    audit(
        &db,
        p.submission_id,
        user_id,
        Some(admin_id),
        status,
        reason,
    )
    .await;

    if !approve {
        for path in [id_image_path, selfie_image_path].into_iter().flatten() {
            if let Err(e) = storage.delete(&path).await {
                tracing::warn!(%path, "kyc image purge failed: {e}");
            }
        }
        audit(
            &db,
            p.submission_id,
            user_id,
            Some(admin_id),
            "images_purged",
            None,
        )
        .await;
    }

    tracing::info!(%admin_id, submission_id = %p.submission_id, status, "kyc reviewed");

    Ok(Json(MessageResponse {
        message: if approve {
            "Submission approved"
        } else {
            "Submission rejected"
        },
    }))
}
