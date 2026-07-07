use argon2::{
    Algorithm, Argon2, Params, PasswordHasher, Version,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::{Extension, Json, http::StatusCode};
use chrono::Utc;
use mongodb::{Database, bson::doc};
use rand::RngExt;
use serde::Deserialize;

use super::shared::{
    E, MessageResponse, hash_code, hash_permit, is_strong_password, is_valid_email,
    normalize_mail_theme, verify_code,
};
use crate::api::mailer;
use crate::api::verified::Verified;

#[derive(Deserialize)]
struct RequestInput {
    email: String,
    /// Mirrors the frontend's current AccentProvider selection so the reset
    /// code email matches whatever theme the user has active.
    #[serde(default)]
    theme: Option<String>,
}

#[derive(Deserialize)]
struct ConfirmInput {
    email: String,
    code: String,
    password: String,
}

#[derive(Deserialize)]
struct ResetDoc {
    code: String,
    expires_at: i64,
    created_at: i64,
    #[serde(default)]
    failed_attempts: i32,
}

/// POST /auth/password-reset/request
///
/// Always returns 200 with the same message regardless of whether the email
/// is registered, to avoid leaking which addresses have accounts. The OTP is
/// only generated and emailed when the account actually exists.
pub async fn request(
    Extension(db): Extension<Database>,
    Verified(msg, _): Verified,
) -> Result<(StatusCode, Json<MessageResponse>), E> {
    let mut p: RequestInput = serde_json::from_slice(&msg)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid payload JSON"))?;
    p.email = p.email.trim().to_ascii_lowercase();

    if !is_valid_email(&p.email) || p.email.len() > 255 {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Invalid email"));
    }
    let theme = normalize_mail_theme(p.theme.as_deref());

    let db_err = |e: mongodb::error::Error| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Reset failed")
    };

    let generic_ok = || {
        Ok((
            StatusCode::OK,
            Json(MessageResponse {
                message: "If that email is registered, a code has been sent",
            }),
        ))
    };

    let now = Utc::now().timestamp();

    let user = db
        .collection::<mongodb::bson::Document>("users")
        .find_one(doc! { "email": &p.email })
        .await
        .map_err(db_err)?;

    if user.is_none() {
        return generic_ok();
    }

    let resets = db.collection::<ResetDoc>("password_reset_codes");
    if let Some(existing) = resets
        .find_one(doc! { "_id": &p.email })
        .await
        .map_err(db_err)?
        && existing.created_at > now - 60
    {
        // Silently accept: same generic 200 as every other outcome, no
        // new code, no email. A 429 here only ever fired for registered
        // emails (reset rows exist only for real accounts), which made
        // the cooldown an account-existence oracle. The earlier code is
        // still valid, so a fast resend loses nothing.
        return generic_ok();
    }

    let code = format!("{:06}", rand::rng().random_range(0..1_000_000u32));
    let expires_at = now + 600;

    // One reset row per email, keyed by `_id`; the upsert replaces any stale
    // row in the same atomic step the old delete-then-insert pair needed a
    // transaction for.
    db.collection::<mongodb::bson::Document>("password_reset_codes")
        .replace_one(
            doc! { "_id": &p.email },
            doc! {
                "_id": &p.email,
                "email": &p.email,
                "code": hash_code(&code),
                "expires_at": expires_at,
                "created_at": now,
                "failed_attempts": 0i32,
            },
        )
        .upsert(true)
        .await
        .map_err(db_err)?;

    // Sent off-task, mirroring register: the mailer round-trip only happens
    // for real accounts, so awaiting it here was a response-latency oracle
    // for which emails are registered. Failures are logged; the user
    // recovers with the resend button.
    let email = p.email.clone();
    tokio::spawn(async move {
        if let Err(e) = mailer::send_code(&email, &code, theme).await {
            tracing::error!("mailer: {e}");
        }
    });

    tracing::info!(email = %p.email, "password reset code sent");
    generic_ok()
}

/// POST /auth/password-reset/confirm
pub async fn confirm(
    Extension(db): Extension<Database>,
    Verified(msg, _): Verified,
) -> Result<(StatusCode, Json<MessageResponse>), E> {
    let mut p: ConfirmInput = serde_json::from_slice(&msg)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid payload JSON"))?;
    p.email = p.email.trim().to_ascii_lowercase();

    if p.code.len() != 6 || !p.code.chars().all(|c| c.is_ascii_digit()) {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Invalid code format"));
    }
    if !is_strong_password(&p.password) {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Password too weak"));
    }

    let db_err = |e: mongodb::error::Error| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Reset failed")
    };

    let now = Utc::now().timestamp();

    let resets = db.collection::<ResetDoc>("password_reset_codes");
    let resets_raw = db.collection::<mongodb::bson::Document>("password_reset_codes");

    resets_raw
        .delete_many(doc! { "expires_at": { "$lte": now } })
        .await
        .map_err(db_err)?;

    let prc = resets
        .find_one(doc! { "_id": &p.email })
        .await
        .map_err(db_err)?
        // same message as a wrong code: reset rows only exist for real accounts,
        // so "no pending reset" after a generic /request response was an
        // account-enumeration oracle
        .ok_or((StatusCode::UNPROCESSABLE_ENTITY, "Incorrect reset code"))?;

    if prc.expires_at <= now {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Reset code has expired"));
    }

    if prc.failed_attempts >= 5 {
        resets_raw
            .delete_one(doc! { "_id": &p.email })
            .await
            .map_err(db_err)?;
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "Too many incorrect attempts. Please request a new code.",
        ));
    }

    if !verify_code(&p.code, &prc.code) {
        let _ = resets_raw
            .update_one(
                doc! { "_id": &p.email },
                doc! { "$inc": { "failed_attempts": 1i32 } },
            )
            .await;
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Incorrect reset code"));
    }

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(65536, 3, 4, None).expect("valid Argon2 params"),
    );
    let permit = hash_permit().await;
    let hash = argon2
        .hash_password(p.password.as_bytes(), &salt)
        .unwrap()
        .to_string();
    drop(permit);

    // Password change, code burn, and session revocation happen together or
    // not at all.
    let mut session = db.client().start_session().await.map_err(db_err)?;
    session.start_transaction().await.map_err(db_err)?;

    let updated = db
        .collection::<mongodb::bson::Document>("users")
        .find_one_and_update(
            doc! { "email": &p.email },
            doc! { "$set": {
                "password_hash": &hash,
                "updated_at": now,
                "failed_login_attempts": 0i32,
                "lockout_until": 0i64,
            } },
        )
        .session(&mut session)
        .await
        .map_err(db_err)?;

    let Some(user) = updated else {
        // only reachable if the account vanished mid-flow; keep it generic
        let _ = session.abort_transaction().await;
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Reset failed"));
    };

    resets_raw
        .delete_one(doc! { "_id": &p.email })
        .session(&mut session)
        .await
        .map_err(db_err)?;

    if let Ok(user_id) = user.get_str("_id") {
        let _ = db
            .collection::<mongodb::bson::Document>("sessions")
            .delete_many(doc! { "user_id": user_id })
            .session(&mut session)
            .await;
    }

    session.commit_transaction().await.map_err(db_err)?;

    tracing::info!(email = %p.email, "password reset successful");
    Ok((
        StatusCode::OK,
        Json(MessageResponse {
            message: "Password updated",
        }),
    ))
}
