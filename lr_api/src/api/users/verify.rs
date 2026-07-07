use axum::{
    Extension, Json,
    http::{HeaderMap, HeaderValue, StatusCode, header::SET_COOKIE},
};
use chrono::Utc;
use mongodb::{Database, bson::doc, options::ReturnDocument};
use serde::Deserialize;
use uuid::Uuid;

use super::shared::{
    E, SESSION_MAX_AGE, UserResponse, clear_legacy_domain_csrf_cookie, csrf_cookie,
    new_csrf_token, session_cookie, verify_code,
};
use crate::api::verified::Verified;

#[derive(Deserialize)]
struct VerifyInput {
    email: String,
    code: String,
}

#[derive(Deserialize)]
struct VcDoc {
    username: String,
    password_hash: String,
    code: String,
    expires_at: i64,
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    failed_attempts: i32,
}

pub async fn verify(
    Extension(db): Extension<Database>,
    Verified(msg, _): Verified,
) -> Result<(StatusCode, HeaderMap, Json<UserResponse>), E> {
    let mut p: VerifyInput =
        serde_json::from_slice(&msg).map_err(|_| (StatusCode::BAD_REQUEST, "Invalid payload"))?;
    p.email = p.email.trim().to_ascii_lowercase();

    if p.code.len() != 6 || !p.code.chars().all(|c| c.is_ascii_digit()) {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Invalid code format"));
    }

    let db_err = |e: mongodb::error::Error| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Verification failed")
    };

    let now = Utc::now().timestamp();

    let codes = db.collection::<VcDoc>("verification_codes");
    let codes_raw = db.collection::<mongodb::bson::Document>("verification_codes");

    // Expired-row sweep, outside the transaction (gc does this too).
    codes_raw
        .delete_many(doc! { "expires_at": { "$lte": now } })
        .await
        .map_err(db_err)?;

    // Everything from here to the session insert is one all-or-nothing
    // transaction, standing in for the old Postgres tx + FOR UPDATE.
    let mut session = db.client().start_session().await.map_err(db_err)?;
    session.start_transaction().await.map_err(db_err)?;

    let vc = codes
        .find_one(doc! { "_id": &p.email })
        .session(&mut session)
        .await
        .map_err(db_err)?
        // same message as a wrong code — "no pending verification" would tell a
        // prober that register silently skipped an already-registered email
        .ok_or((
            StatusCode::UNPROCESSABLE_ENTITY,
            "Incorrect verification code",
        ))?;

    if vc.expires_at <= now {
        let _ = session.abort_transaction().await;
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "Verification code has expired",
        ));
    }

    if vc.failed_attempts >= 5 {
        codes_raw
            .delete_one(doc! { "_id": &p.email })
            .session(&mut session)
            .await
            .map_err(db_err)?;
        session.commit_transaction().await.ok();
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "Too many incorrect attempts. Please request a new code.",
        ));
    }

    if !verify_code(&p.code, &vc.code) {
        let _ = codes_raw
            .update_one(
                doc! { "_id": &p.email },
                doc! { "$inc": { "failed_attempts": 1i32 } },
            )
            .session(&mut session)
            .await;
        session.commit_transaction().await.ok();
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "Incorrect verification code",
        ));
    }

    let user_id = Uuid::new_v4();

    // Re-check the access token now rather than trusting the register-time
    // validation: up to 10 minutes pass between the two, and a token revoked
    // (or expired, or redeemed by someone else) in that window must not still
    // buy the elevated role. The check and the redemption are one atomic
    // find-and-update; if account creation fails below, the transaction
    // rolls the redemption back. Falls back to Pending, never fails the signup.
    let token_still_valid = match vc.access_token.as_deref().filter(|t| !t.is_empty()) {
        None => false,
        Some(token) => db
            .collection::<mongodb::bson::Document>("access_tokens")
            .find_one_and_update(
                doc! {
                    "_id": token,
                    "revoked_at": null,
                    "redeemed_at": null,
                    "$or": [
                        { "expires_at": null },
                        { "expires_at": { "$gt": now } },
                    ],
                },
                doc! { "$set": { "redeemed_by": user_id.to_string(), "redeemed_at": now } },
            )
            .return_document(ReturnDocument::After)
            .session(&mut session)
            .await
            .map_err(db_err)?
            .is_some(),
    };
    let role = if token_still_valid { "User" } else { "Pending" };

    if let Err(e) = db
        .collection::<mongodb::bson::Document>("users")
        .insert_one(doc! {
            "_id": user_id.to_string(),
            "username": &vc.username,
            "email": &p.email,
            "password_hash": &vc.password_hash,
            "role": role,
            "failed_login_attempts": 0i32,
            "lockout_until": 0i64,
            "created_at": now,
            "updated_at": now,
        })
        .session(&mut session)
        .await
    {
        tracing::error!("DB: {e}");
        let _ = session.abort_transaction().await;
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "Account creation failed"));
    }

    codes_raw
        .delete_many(doc! { "$or": [
            { "_id": &p.email },
            { "expires_at": { "$lte": now } },
        ] })
        .session(&mut session)
        .await
        .map_err(db_err)?;

    db.collection::<mongodb::bson::Document>("sessions")
        .delete_many(doc! { "$or": [
            { "user_id": user_id.to_string() },
            { "expires_at": { "$lte": now } },
        ] })
        .session(&mut session)
        .await
        .map_err(db_err)?;

    let session_id = Uuid::new_v4();
    db.collection::<mongodb::bson::Document>("sessions")
        .insert_one(doc! {
            "_id": session_id.to_string(),
            "user_id": user_id.to_string(),
            "created_at": now,
            "expires_at": now + SESSION_MAX_AGE,
        })
        .session(&mut session)
        .await
        .map_err(db_err)?;

    session.commit_transaction().await.map_err(|e| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Account creation failed")
    })?;

    tracing::info!(email = %p.email, %user_id, "user registered");

    let mut headers = HeaderMap::new();
    let csrf_token = new_csrf_token();
    headers.append(SET_COOKIE, session_cookie(session_id));
    headers.append(SET_COOKIE, csrf_cookie(&csrf_token));
    if let Some(clear) = clear_legacy_domain_csrf_cookie() {
        headers.append(SET_COOKIE, clear);
    }
    headers.insert(
        axum::http::header::HeaderName::from_static("x-csrf-token"),
        HeaderValue::from_str(&csrf_token).expect("valid csrf token"),
    );
    Ok((
        StatusCode::CREATED,
        headers,
        Json(UserResponse {
            id: user_id,
            username: vc.username,
            email: p.email,
            role: role.into(),
            expires_at: now + SESSION_MAX_AGE,
        }),
    ))
}
