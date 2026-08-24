use axum::{
    Extension, Json,
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use mongodb::{
    Database,
    bson::{Binary, doc, spec::BinarySubtype},
};
use serde::Deserialize;
use uuid::Uuid;

use super::shared::{
    ID_TYPES, MAX_DOB_LEN, MAX_ID_NUMBER_LEN, MAX_NAME_LEN, audit, decode_image,
    is_valid_stellar_address,
};
use crate::api::users::shared::{E, MessageResponse, require_user};
use crate::infra::db::is_duplicate_key;
use crate::infra::{crypto, storage::MongoStorage};

#[derive(Deserialize)]
pub struct SubmitInput {
    id_type: String,
    first_name: String,
    #[serde(default)]
    middle_name: String,
    last_name: String,
    dob: String,
    id_number: String,
    wallet_address: String,
    #[serde(default)]
    face_match_score: Option<i16>,
    #[serde(default)]
    liveness_passed: bool,
    /// Raw base64 or data: URLs; real type is sniffed server-side.
    id_image: String,
    selfie_image: String,
}

fn required_field(value: &str, max: usize, label: &'static str) -> Result<String, E> {
    let v = value.trim();
    if v.is_empty() || v.len() > max {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, label));
    }
    Ok(v.to_string())
}

pub async fn submit(
    Extension(db): Extension<Database>,
    Extension(storage): Extension<MongoStorage>,
    headers: HeaderMap,
    Json(p): Json<SubmitInput>,
) -> Result<(StatusCode, Json<MessageResponse>), E> {
    let user_id = require_user(&db, &headers).await?;

    // Fail closed: without the encryption key or a storage target there is no
    // acceptable degraded mode for identity documents.
    if !crypto::is_configured() {
        tracing::error!("KYC submit refused: KYC_ENC_KEY is not configured");
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Identity verification is temporarily unavailable",
        ));
    }
    if !storage.is_configured() {
        tracing::error!("KYC submit refused: document storage is not configured");
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Identity verification is temporarily unavailable",
        ));
    }

    // ---- field validation ----
    let id_type = p.id_type.trim().to_ascii_lowercase();
    if !ID_TYPES.contains(&id_type.as_str()) {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Unknown ID type"));
    }
    let first_name = required_field(&p.first_name, MAX_NAME_LEN, "Invalid first name")?;
    let last_name = required_field(&p.last_name, MAX_NAME_LEN, "Invalid last name")?;
    let middle_name = p.middle_name.trim().to_string();
    if middle_name.len() > MAX_NAME_LEN {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Invalid middle name"));
    }
    let dob = required_field(&p.dob, MAX_DOB_LEN, "Invalid date of birth")?;
    let id_number = required_field(&p.id_number, MAX_ID_NUMBER_LEN, "Invalid ID number")?;
    if id_number.chars().filter(|c| c.is_ascii_alphanumeric()).count() < 4 {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Invalid ID number"));
    }
    let wallet_address = p.wallet_address.trim().to_string();
    if !is_valid_stellar_address(&wallet_address) {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Invalid wallet address"));
    }
    if let Some(score) = p.face_match_score
        && !(0..=100).contains(&score)
    {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Invalid match score"));
    }
    // ---- images: decode + sniff before anything is stored ----
    let (id_bytes, id_ext, id_mime) =
        decode_image(&p.id_image).map_err(|m| (StatusCode::UNPROCESSABLE_ENTITY, m))?;
    let (selfie_bytes, selfie_ext, selfie_mime) =
        decode_image(&p.selfie_image).map_err(|m| (StatusCode::UNPROCESSABLE_ENTITY, m))?;

    let submissions = db.collection::<mongodb::bson::Document>("kyc_submissions");
    let live = doc! { "$in": ["pending", "approved"] };

    // ---- duplicate checks (fast fail before uploading megabytes) ----
    // The partial unique indexes remain the source of truth under races; these
    // exist to answer politely instead of via constraint violation.
    let existing = submissions
        .find_one(doc! { "user_id": user_id.to_string(), "status": live.clone() })
        .await
        .map_err(|e| {
            tracing::error!("DB kyc existing: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Submission failed")
        })?;
    match existing.as_ref().and_then(|d| d.get_str("status").ok()) {
        Some("approved") => {
            return Err((StatusCode::CONFLICT, "Your identity is already verified"));
        }
        Some(_) => {
            return Err((
                StatusCode::CONFLICT,
                "Your submission is already under review",
            ));
        }
        None => {}
    }

    let id_number_hash = crypto::blind_index(&id_number);
    let id_taken = submissions
        .find_one(doc! { "id_number_hash": &id_number_hash, "status": live })
        .await
        .map_err(|e| {
            tracing::error!("DB kyc dedupe: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Submission failed")
        })?;
    if id_taken.is_some() {
        // deliberately vague: don't confirm to a prober that this ID number
        // exists on another account
        return Err((
            StatusCode::CONFLICT,
            "This ID cannot be used for verification",
        ));
    }

    // ---- encrypt PII ----
    let seal = |value: &str| {
        crypto::seal(value).map_err(|e| {
            tracing::error!("KYC seal failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Submission failed")
        })
    };
    let bin = |bytes: Vec<u8>| Binary {
        subtype: BinarySubtype::Generic,
        bytes,
    };
    let first_name_enc = bin(seal(&first_name)?);
    let middle_name_enc = if middle_name.is_empty() {
        None
    } else {
        Some(bin(seal(&middle_name)?))
    };
    let last_name_enc = bin(seal(&last_name)?);
    let dob_enc = bin(seal(&dob)?);
    let id_number_enc = bin(seal(&id_number)?);

    // ---- store documents, then the row; clean up storage if the row fails ----
    let submission_id = Uuid::new_v4();
    let id_image_path = format!("{user_id}/{submission_id}/id.{id_ext}");
    let selfie_image_path = format!("{user_id}/{submission_id}/selfie.{selfie_ext}");

    storage
        .upload_private(&id_image_path, id_bytes, id_mime)
        .await
        .map_err(|e| {
            tracing::error!("KYC storage (id): {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Unable to store documents")
        })?;
    if let Err(e) = storage
        .upload_private(&selfie_image_path, selfie_bytes, selfie_mime)
        .await
    {
        tracing::error!("KYC storage (selfie): {e}");
        let _ = storage.delete(&id_image_path).await;
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "Unable to store documents"));
    }

    let now = Utc::now().timestamp();
    let inserted = submissions
        .insert_one(doc! {
            "_id": submission_id.to_string(),
            "user_id": user_id.to_string(),
            "status": "pending",
            "id_type": &id_type,
            "first_name_enc": first_name_enc,
            "middle_name_enc": middle_name_enc,
            "last_name_enc": last_name_enc,
            "dob_enc": dob_enc,
            "id_number_enc": id_number_enc,
            "id_number_hash": &id_number_hash,
            "wallet_address": &wallet_address,
            "face_match_score": p.face_match_score.map(i32::from),
            "liveness_passed": p.liveness_passed,
            "id_image_path": &id_image_path,
            "selfie_image_path": &selfie_image_path,
            "created_at": now,
            "updated_at": now,
        })
        .await;

    if let Err(e) = inserted {
        let _ = storage.delete(&id_image_path).await;
        let _ = storage.delete(&selfie_image_path).await;
        // a race lost to the partial unique indexes (double-submit, or the
        // same ID number landing on two accounts simultaneously)
        if is_duplicate_key(&e) {
            return Err((
                StatusCode::CONFLICT,
                "This ID cannot be used for verification",
            ));
        }
        tracing::error!("DB kyc insert: {e}");
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "Submission failed"));
    }

    audit(&db, submission_id, user_id, Some(user_id), "submitted", None).await;
    tracing::info!(%user_id, %submission_id, "kyc submitted");

    Ok((
        StatusCode::CREATED,
        Json(MessageResponse {
            message: "Identity verification submitted for review",
        }),
    ))
}
