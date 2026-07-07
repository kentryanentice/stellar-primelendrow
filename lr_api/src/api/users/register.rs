use argon2::{
    Algorithm, Argon2, Params, PasswordHasher, PasswordVerifier, Version,
    password_hash::{PasswordHash, SaltString, rand_core::OsRng},
};
use axum::{Extension, Json, http::StatusCode};
use chrono::Utc;
use mongodb::{Database, bson::doc};
use rand::RngExt;
use serde::Deserialize;

use super::shared::{
    E, MessageResponse, hash_code, hash_permit, is_strong_password, is_valid_email,
    normalize_mail_theme,
};
use crate::api::mailer;
use crate::api::verified::Verified;
use crate::infra::db::is_duplicate_key;

#[derive(Deserialize)]
struct RegisterInput {
    name: String,
    email: String,
    password: String,
    access_token: Option<String>,
    /// Mirrors the frontend's current AccentProvider selection so the OTP
    /// email matches whatever theme the user has active; see
    /// `normalize_mail_theme` for how out-of-range values are handled.
    #[serde(default)]
    theme: Option<String>,
}

#[derive(Deserialize)]
struct TokenDoc {
    #[serde(default)]
    reserved_email: Option<String>,
    #[serde(default)]
    reserved_at: Option<i64>,
    #[serde(default)]
    redeemed_at: Option<i64>,
    #[serde(default)]
    revoked_at: Option<i64>,
    #[serde(default)]
    expires_at: Option<i64>,
}

#[derive(Deserialize)]
struct PendingDoc {
    password_hash: String,
    expires_at: i64,
    created_at: i64,
}

pub async fn register(
    Extension(db): Extension<Database>,
    Verified(msg, _): Verified,
) -> Result<(StatusCode, Json<MessageResponse>), E> {
    let mut p: RegisterInput = serde_json::from_slice(&msg)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid payload JSON"))?;
    p.email = p.email.trim().to_ascii_lowercase();

    if !is_valid_email(&p.email) {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Invalid email"));
    }
    if p.name.is_empty() || p.name.len() > 255 {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Name too long"));
    }
    if p.email.len() > 255 {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Email too long"));
    }
    if !is_strong_password(&p.password) {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Password too weak"));
    }
    let access_token = p.access_token.unwrap_or_default().trim().to_string();
    if access_token.len() > 100 {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "Invalid access token"));
    }
    let theme = normalize_mail_theme(p.theme.as_deref());

    let db_err = |e: mongodb::error::Error| {
        tracing::error!("DB: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Registration failed")
    };

    let now = Utc::now().timestamp();
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
    let code = format!("{:06}", rand::rng().random_range(0..1_000_000u32));
    let expires_at = now + 600;

    // Indistinguishable-from-success response used by every "don't tell a
    // prober anything" path below.
    let generic_created = || {
        Ok((
            StatusCode::CREATED,
            Json(MessageResponse {
                message: "Verification code sent",
            }),
        ))
    };

    if db
        .collection::<mongodb::bson::Document>("users")
        .find_one(doc! { "email": &p.email })
        .await
        .map_err(db_err)?
        .is_some()
    {
        // Indistinguishable from success so the register endpoint can't be
        // used to test which emails have accounts (409 here was an oracle).
        // No code is stored or emailed; a later /auth/verify just fails like
        // any wrong code. The password was already hashed above, so timing
        // stays comparable to the real path minus the mailer call.
        tracing::warn!(email = %p.email, "register attempt for existing email");
        return generic_created();
    }

    let access_token_opt: Option<String> = if access_token.is_empty() {
        None
    } else {
        Some(access_token.clone())
    };

    if let Some(ref token) = access_token_opt {
        let tokens = db.collection::<TokenDoc>("access_tokens");
        let token_row = tokens
            .find_one(doc! { "_id": token })
            .await
            .map_err(db_err)?
            .ok_or((StatusCode::UNPROCESSABLE_ENTITY, "Invalid access token"))?;

        if token_row.revoked_at.is_some() {
            return Err((StatusCode::UNPROCESSABLE_ENTITY, "Invalid access token"));
        }
        if token_row.redeemed_at.is_some() {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                "Access token already used",
            ));
        }
        if token_row.expires_at.is_some_and(|exp| exp <= now) {
            return Err((StatusCode::UNPROCESSABLE_ENTITY, "Access token has expired"));
        }

        if let (Some(reserved_email), Some(reserved_at)) =
            (token_row.reserved_email.as_deref(), token_row.reserved_at)
            && reserved_at >= now - 600
            && reserved_email != p.email
        {
            return Err((StatusCode::CONFLICT, "Access token already reserved"));
        }

        tokens
            .update_one(
                // re-assert validity in the filter so a token revoked or
                // redeemed since the read can't be re-reserved (the atomic
                // update stands in for the old SELECT ... FOR UPDATE)
                doc! { "_id": token, "revoked_at": null, "redeemed_at": null },
                doc! { "$set": { "reserved_email": &p.email, "reserved_at": now } },
            )
            .await
            .map_err(db_err)?;
    }

    // Pending registrations are keyed by email (`_id`), so "one pending
    // signup per email" is enforced by the collection itself.
    let pending = db.collection::<PendingDoc>("verification_codes");
    let existing = pending
        .find_one(doc! { "_id": &p.email })
        .await
        .map_err(db_err)?;

    let new_doc = doc! {
        "_id": &p.email,
        "username": &p.name,
        "email": &p.email,
        "password_hash": &hash,
        "code": hash_code(&code),
        "expires_at": expires_at,
        "created_at": now,
        "failed_attempts": 0i32,
        "access_token": access_token_opt.as_deref(),
    };

    if let Some(existing) = existing {
        if existing.expires_at > now && existing.created_at > now - 60 {
            // Silently accept: same generic success as the existing-account
            // path, no new code, no email. A 429 here only ever fired while a
            // pending (i.e. *unregistered*) signup existed — the mirror image
            // of the existing-email fake success — so together they formed an
            // account-existence oracle. The earlier code is still valid.
            tracing::info!(email = %p.email, "register attempt within resend cooldown");
            return generic_created();
        }
        // While a pending registration is still live, only the original
        // registrant (proven by knowing its password — the resend button
        // re-posts the identical payload) may replace it. Otherwise anyone
        // could swap in their own password after the 60s cooldown and hijack
        // the account the moment the victim enters the fresher emailed code.
        // Answered with the same generic success as the existing-account
        // path so it can't be used to probe for pending registrations.
        if existing.expires_at > now {
            let permit = hash_permit().await;
            let same_password = PasswordHash::new(&existing.password_hash).is_ok_and(|parsed| {
                argon2
                    .verify_password(p.password.as_bytes(), &parsed)
                    .is_ok()
            });
            drop(permit);
            if !same_password {
                tracing::warn!(email = %p.email, "register attempt while a different pending registration exists");
                return generic_created();
            }
        }
        // Replace only the exact pending doc we examined (matched on
        // created_at): if a concurrent register already swapped it, answer
        // with the generic success instead of clobbering the fresher one.
        let replaced = db
            .collection::<mongodb::bson::Document>("verification_codes")
            .replace_one(
                doc! { "_id": &p.email, "created_at": existing.created_at },
                new_doc,
            )
            .await
            .map_err(db_err)?;
        if replaced.modified_count == 0 {
            tracing::info!(email = %p.email, "register raced a concurrent pending replacement");
            return generic_created();
        }
    } else {
        match db
            .collection::<mongodb::bson::Document>("verification_codes")
            .insert_one(new_doc)
            .await
        {
            Ok(_) => {}
            // a concurrent register for the same email won the insert
            Err(e) if is_duplicate_key(&e) => {
                tracing::info!(email = %p.email, "register raced a concurrent pending insert");
                return generic_created();
            }
            Err(e) => return Err(db_err(e)),
        }
    }

    // Sent off-task: the mailer's network round-trip was a timing oracle —
    // the existing-email path above (which sends nothing) answered measurably
    // faster than this one. Failures are logged, and the user recovers with
    // the resend button.
    let email = p.email.clone();
    tokio::spawn(async move {
        if let Err(e) = mailer::send_code(&email, &code, theme).await {
            tracing::error!("mailer: {e}");
        }
    });

    tracing::info!(email = %p.email, "verification code sent");
    generic_created()
}
