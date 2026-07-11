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

// ---- pending list: paginated, includes signed thumbnail URLs ----
//
// No PII text (names/DOB/ID number stay encrypted here — only `detail` below
// decrypts and audits that). Images are a deliberate exception: the review
// queue's UI shows selfie/ID thumbnails on every card, so every visible page
// mints short-lived signed URLs for it up front rather than one at a time.

fn default_page() -> i64 {
    1
}
/// Fixed server-side — the client only ever picks *which* page, never *how
/// large*, so there's no page_size for a caller to inflate into "dump the
/// whole table".
const PAGE_SIZE: i64 = 10;

#[derive(Deserialize)]
pub struct PendingRequest {
    #[serde(default = "default_page")]
    page: i64,
}

#[derive(Serialize)]
pub struct PendingItem {
    pub id: Uuid,
    pub user_id: Uuid,
    pub id_type: String,
    pub face_match_score: Option<i16>,
    pub liveness_passed: bool,
    pub created_at: i64,
    /// Signed, short-lived (~5 min) thumbnail URLs; None if the path is
    /// missing or signing failed (degrades to no thumbnail, not a page error).
    pub id_image_url: Option<String>,
    pub selfie_image_url: Option<String>,
}

#[derive(Serialize)]
pub struct PendingResponse {
    pub items: Vec<PendingItem>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}

pub async fn pending(
    Extension(pool): Extension<PgPool>,
    Extension(storage): Extension<SupabaseStorage>,
    headers: HeaderMap,
    Json(q): Json<PendingRequest>,
) -> Result<Json<PendingResponse>, E> {
    require_admin(&pool, &headers).await?;

    // clamp rather than reject: a caller passing page=0 gets a sane response
    // instead of a 4xx round-trip
    let page = q.page.max(1);
    let page_size = PAGE_SIZE;
    let offset = (page - 1) * page_size;

    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM public.kyc_submissions WHERE status = 'verifying'",
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        tracing::error!("DB kyc pending count: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Unable to load queue")
    })?;

    let rows: Vec<(Uuid, Uuid, String, Option<i16>, bool, i64, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, user_id, id_type, face_match_score, liveness_passed, created_at,
                id_image_path, selfie_image_path
           FROM public.kyc_submissions
          WHERE status = 'verifying'
          ORDER BY created_at ASC
          LIMIT $1 OFFSET $2",
    )
    .bind(page_size)
    .bind(offset)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        tracing::error!("DB kyc pending: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Unable to load queue")
    })?;

    // sign both thumbnails for every row concurrently — sequentially awaiting
    // up to page_size * 2 Supabase API calls would make a full page take
    // seconds instead of the length of the single slowest call
    let mut signing = tokio::task::JoinSet::new();
    for (idx, row) in rows.iter().enumerate() {
        let storage = storage.clone();
        let id_path = row.6.clone();
        let selfie_path = row.7.clone();
        signing.spawn(async move {
            let sign = |path: Option<String>| {
                let storage = storage.clone();
                async move {
                    match path {
                        None => None,
                        Some(p) => storage.signed_url(&p, SIGNED_URL_TTL_SECS).await.ok(),
                    }
                }
            };
            let id_url = sign(id_path).await;
            let selfie_url = sign(selfie_path).await;
            (idx, id_url, selfie_url)
        });
    }
    let mut urls: Vec<(Option<String>, Option<String>)> = vec![(None, None); rows.len()];
    while let Some(res) = signing.join_next().await {
        if let Ok((idx, id_url, selfie_url)) = res {
            urls[idx] = (id_url, selfie_url);
        }
    }

    let items = rows
        .into_iter()
        .zip(urls)
        .map(
            |((id, user_id, id_type, face_match_score, liveness_passed, created_at, _, _), (id_image_url, selfie_image_url))| PendingItem {
                id,
                user_id,
                id_type,
                face_match_score,
                liveness_passed,
                created_at,
                id_image_url,
                selfie_image_url,
            },
        )
        .collect();

    let total_pages = if total == 0 {
        1
    } else {
        (total + page_size - 1) / page_size
    };

    Ok(Json(PendingResponse {
        items,
        total,
        page,
        page_size,
        total_pages,
    }))
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
    let row: Option<(Uuid, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "UPDATE public.kyc_submissions
            SET status = $1, reviewed_by = $2, reviewed_at = $3,
                rejection_reason = $4, updated_at = $3
          WHERE id = $5 AND status = 'verifying'
          RETURNING user_id, id_image_path, selfie_image_path, wallet_address",
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

    let (user_id, id_image_path, selfie_image_path, wallet_address) =
        row.ok_or((StatusCode::NOT_FOUND, "No pending submission with that id"))?;

    if approve {
        // verified identity unlocks the account: Verifying -> User. Admins keep
        // their role; the guard also stops a demotion if roles ever grow.
        sqlx::query(
            "UPDATE public.users SET role = 'User', updated_at = $1
              WHERE id = $2 AND role = 'Verifying'",
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
        // rejected: back to Verifying's peer state, Pending — free to resubmit.
        // Guarded the same way so it never touches an already-promoted User/Admin.
        sqlx::query(
            "UPDATE public.users SET role = 'Pending', updated_at = $1
              WHERE id = $2 AND role = 'Verifying'",
        )
        .bind(now)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!("DB kyc demote: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Review failed")
        })?;

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

    // Best-effort: seed the reviewed wallet as this account's anchor row in
    // the live `wallets` table (api::wallets). Deliberately outside the
    // approval transaction and non-fatal on failure — the approval itself
    // (status + role, already committed above) must never be blocked or
    // rolled back by a hiccup in this side effect, same rationale as
    // audit()'s own errors-are-logged-not-propagated policy. Failure here
    // just means the account has no anchor wallet yet; it can still connect
    // one from Settings once the underlying issue (e.g. a pending
    // migration) is fixed. kyc_submissions.wallet_address itself is never
    // touched here or anywhere else — this only ever copies it. ON CONFLICT
    // DO NOTHING (no target) absorbs a conflict against either of that
    // table's unique indexes: a resubmission reusing the same (user,
    // address), or the address already being someone else's active wallet.
    if approve && let Some(wallet_address) = &wallet_address {
        if let Err(e) = sqlx::query(
            "INSERT INTO public.wallets
                (user_id, address, source, status, connected_at, created_at, updated_at)
             VALUES ($1, $2, 'kyc_verified', 'active', $3, $3, $3)
             ON CONFLICT DO NOTHING",
        )
        .bind(user_id)
        .bind(wallet_address)
        .bind(now)
        .execute(&pool)
        .await
        {
            tracing::error!(%user_id, "kyc wallet seed failed: {e}");
        }
    }

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
