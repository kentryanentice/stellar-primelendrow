use axum::{
    Extension, Json,
    extract::Path,
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::shared::{MAX_REASON_LEN, audit};
use crate::api::users::shared::{E, MessageResponse, require_admin};
use crate::infra::{crypto, storage::SupabaseStorage};

/// How long an admin's signed image URL stays valid. Long enough to review,
/// short enough that a leaked URL from a screen-share is soon worthless.
const SIGNED_URL_TTL_SECS: u32 = 5 * 60;

// ---- pending list: metadata only, no PII ----
//
// Listing is cheap to poll, so it must not decrypt anything; PII access is a
// deliberate, per-submission, audited act via `detail` below.

#[derive(Serialize)]
pub struct PendingItem {
    pub id: Uuid,
    pub user_id: Uuid,
    pub id_type: String,
    pub face_match_score: Option<i16>,
    pub liveness_passed: bool,
    pub created_at: i64,
}

pub async fn pending(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
) -> Result<Json<Vec<PendingItem>>, E> {
    require_admin(&pool, &headers).await?;

    let rows: Vec<(Uuid, Uuid, String, Option<i16>, bool, i64)> = sqlx::query_as(
        "SELECT id, user_id, id_type, face_match_score, liveness_passed, created_at
           FROM public.kyc_submissions
          WHERE status = 'pending'
          ORDER BY created_at ASC
          LIMIT 100",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        tracing::error!("DB kyc pending: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Unable to load queue")
    })?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(id, user_id, id_type, face_match_score, liveness_passed, created_at)| {
                    PendingItem {
                        id,
                        user_id,
                        id_type,
                        face_match_score,
                        liveness_passed,
                        created_at,
                    }
                },
            )
            .collect(),
    ))
}

// ---- detail: decrypted PII + short-lived signed image URLs, audited ----

#[derive(Serialize)]
pub struct SubmissionDetail {
    pub id: Uuid,
    pub user_id: Uuid,
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

// A named row struct rather than a tuple: sqlx only implements FromRow for
// tuples up to 16 columns.
#[derive(sqlx::FromRow)]
struct DetailRow {
    user_id: Uuid,
    status: String,
    id_type: String,
    first_name_enc: Vec<u8>,
    middle_name_enc: Option<Vec<u8>>,
    last_name_enc: Vec<u8>,
    dob_enc: Vec<u8>,
    id_number_enc: Vec<u8>,
    wallet_address: Option<String>,
    face_match_score: Option<i16>,
    liveness_passed: bool,
    id_image_path: Option<String>,
    selfie_image_path: Option<String>,
    rejection_reason: Option<String>,
    created_at: i64,
    reviewed_at: Option<i64>,
}

pub async fn detail(
    Extension(pool): Extension<PgPool>,
    Extension(storage): Extension<SupabaseStorage>,
    headers: HeaderMap,
    Path(submission_id): Path<Uuid>,
) -> Result<Json<SubmissionDetail>, E> {
    let admin_id = require_admin(&pool, &headers).await?;

    let row: Option<DetailRow> = sqlx::query_as(
        "SELECT user_id, status, id_type,
                first_name_enc, middle_name_enc, last_name_enc, dob_enc, id_number_enc,
                wallet_address, face_match_score, liveness_passed,
                id_image_path, selfie_image_path,
                rejection_reason, created_at, reviewed_at
           FROM public.kyc_submissions
          WHERE id = $1",
    )
    .bind(submission_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        tracing::error!("DB kyc detail: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Unable to load submission")
    })?;

    let row = row.ok_or((StatusCode::NOT_FOUND, "Submission not found"))?;
    let user_id = row.user_id;

    let open = |sealed: &[u8]| {
        crypto::open(sealed).map_err(|e| {
            tracing::error!(%submission_id, "KYC decrypt failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Unable to load submission")
        })
    };
    let first_name = open(&row.first_name_enc)?;
    let middle_name = row.middle_name_enc.as_deref().map(open).transpose()?;
    let last_name = open(&row.last_name_enc)?;
    let dob = open(&row.dob_enc)?;
    let id_number = open(&row.id_number_enc)?;

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
        &pool,
        submission_id,
        user_id,
        Some(admin_id),
        "viewed",
        Some("decrypted PII and signed image URLs"),
    )
    .await;

    Ok(Json(SubmissionDetail {
        id: submission_id,
        user_id,
        status: row.status,
        id_type: row.id_type,
        first_name,
        middle_name,
        last_name,
        dob,
        id_number,
        wallet_address: row.wallet_address,
        face_match_score: row.face_match_score,
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
    Extension(pool): Extension<PgPool>,
    Extension(storage): Extension<SupabaseStorage>,
    headers: HeaderMap,
    Json(p): Json<ReviewInput>,
) -> Result<Json<MessageResponse>, E> {
    let admin_id = require_admin(&pool, &headers).await?;

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

    let now = Utc::now().timestamp();
    let status = if approve { "approved" } else { "rejected" };

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Review failed")
    })?;

    // status guard in the WHERE clause makes the decision idempotent and
    // race-safe: two admins deciding at once — exactly one wins
    let row: Option<(Uuid, Option<String>, Option<String>)> = sqlx::query_as(
        "UPDATE public.kyc_submissions
            SET status = $1, reviewed_by = $2, reviewed_at = $3,
                rejection_reason = $4, updated_at = $3
          WHERE id = $5 AND status = 'pending'
          RETURNING user_id, id_image_path, selfie_image_path",
    )
    .bind(status)
    .bind(admin_id)
    .bind(now)
    .bind(if approve { None } else { reason })
    .bind(p.submission_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("DB kyc review: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Review failed")
    })?;

    let (user_id, id_image_path, selfie_image_path) =
        row.ok_or((StatusCode::NOT_FOUND, "No pending submission with that id"))?;

    if approve {
        // verified identity unlocks the account: Pending -> User. Admins keep
        // their role; the guard also stops a demotion if roles ever grow.
        sqlx::query(
            "UPDATE public.users SET role = 'User', updated_at = $1
              WHERE id = $2 AND role = 'Pending'",
        )
        .bind(now)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!("DB kyc promote: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Review failed")
        })?;
    } else {
        // data minimization: a rejected applicant's documents have no reason
        // to stay on file — clear the paths now, delete the objects after
        // commit (storage delete is best-effort/idempotent)
        sqlx::query(
            "UPDATE public.kyc_submissions
                SET id_image_path = NULL, selfie_image_path = NULL
              WHERE id = $1",
        )
        .bind(p.submission_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!("DB kyc purge paths: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Review failed")
        })?;
    }

    tx.commit().await.map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Review failed")
    })?;

    audit(
        &pool,
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
            &pool,
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
