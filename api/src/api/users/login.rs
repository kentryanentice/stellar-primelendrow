use argon2::{
    Algorithm, Argon2, Params, PasswordHasher, PasswordVerifier, Version,
    password_hash::{PasswordHash, SaltString, rand_core::OsRng},
};
use axum::{
    Extension, Json,
    extract::ConnectInfo,
    http::{HeaderMap, HeaderValue, StatusCode, header::SET_COOKIE},
};
use chrono::Utc;
use mongodb::{Database, bson::doc};
use serde::Deserialize;
use std::net::SocketAddr;
use uuid::Uuid;

use super::shared::{
    E, MAX_PASSWORD_LEN, SESSION_MAX_AGE, UserResponse, clear_legacy_domain_csrf_cookie,
    csrf_cookie, hash_permit, is_valid_email, new_csrf_token, session_cookie,
};
use crate::api::verified::Verified;
use crate::infra::{login_guard, rate};

// Account-wide lockout is only a backstop against *distributed* brute force
// (many IPs, one account) — the primary defense is the per-(IP, email) guard
// in `infra::login_guard`, which an attacker can't use to lock out the real
// account owner. Keeping this threshold low would reintroduce exactly that
// lockout-DoS: 5 remote failures used to freeze the account for everyone.
const MAX_FAILED_ATTEMPTS: i32 = 30;
const LOCKOUT_SECONDS: i64 = 15 * 60;

#[derive(Deserialize)]
struct LoginInput {
    email: String,
    password: String,
}

#[derive(Deserialize)]
struct UserDoc {
    #[serde(rename = "_id")]
    id: String,
    username: String,
    email: String,
    password_hash: String,
    role: String,
    #[serde(default)]
    failed_login_attempts: i32,
    #[serde(default)]
    lockout_until: i64,
}

pub async fn login(
    Extension(db): Extension<Database>,
    connect_info: ConnectInfo<SocketAddr>,
    request_headers: HeaderMap,
    Verified(msg, _): Verified,
) -> Result<(StatusCode, HeaderMap, Json<UserResponse>), E> {
    let mut p: LoginInput =
        serde_json::from_slice(&msg).map_err(|_| (StatusCode::BAD_REQUEST, "Invalid payload"))?;
    p.email = p.email.trim().to_ascii_lowercase();

    if !is_valid_email(&p.email) || p.email.len() > 255 || p.password.len() > MAX_PASSWORD_LEN {
        return Err((StatusCode::UNAUTHORIZED, "Invalid email or password"));
    }

    let now = Utc::now().timestamp();

    // Checked before touching the DB, and for unknown emails too — a probing
    // client sees the same 429 whether or not the account exists.
    let client_ip = rate::extract_ip(&request_headers, Some(&connect_info))
        .ok_or((StatusCode::FORBIDDEN, "Unable to determine client"))?;
    if login_guard::is_locked(client_ip, &p.email, now) {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "Too many failed attempts. Please try again later.",
        ));
    }

    let users = db.collection::<UserDoc>("users");
    let users_raw = db.collection::<mongodb::bson::Document>("users");

    let row = match users
        .find_one(doc! { "email": &p.email })
        .await
        .map_err(|e| {
            tracing::error!("DB: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Login failed")
        })? {
        Some(row) => row,
        None => {
            // Burn a comparable amount of time hashing so a missing account
            // can't be distinguished from a wrong password by response latency.
            let argon2 = Argon2::new(
                Algorithm::Argon2id,
                Version::V0x13,
                Params::new(65536, 3, 4, None).expect("valid Argon2 params"),
            );
            let permit = hash_permit().await;
            let _ = argon2.hash_password(p.password.as_bytes(), &SaltString::generate(&mut OsRng));
            drop(permit);
            login_guard::record_failure(client_ip, &p.email, now);
            return Err((StatusCode::UNAUTHORIZED, "Invalid email or password"));
        }
    };

    if row.lockout_until > now {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "Too many failed attempts. Please try again later.",
        ));
    }

    let parsed = PasswordHash::new(&row.password_hash)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Login failed"))?;

    let permit = hash_permit().await;
    let password_ok = Argon2::default()
        .verify_password(p.password.as_bytes(), &parsed)
        .is_ok();
    drop(permit);

    if !password_ok {
        login_guard::record_failure(client_ip, &p.email, now);
        let new_count = row.failed_login_attempts + 1;
        if new_count >= MAX_FAILED_ATTEMPTS {
            let _ = users_raw
                .update_one(
                    doc! { "_id": &row.id },
                    doc! { "$set": {
                        "failed_login_attempts": 0i32,
                        "lockout_until": now + LOCKOUT_SECONDS,
                    } },
                )
                .await;
            tracing::warn!(email = %p.email, "account locked after repeated failed logins");
        } else {
            let _ = users_raw
                .update_one(
                    doc! { "_id": &row.id },
                    doc! { "$set": { "failed_login_attempts": new_count } },
                )
                .await;
        }
        return Err((StatusCode::UNAUTHORIZED, "Invalid email or password"));
    }

    login_guard::clear(client_ip, &p.email);

    if row.failed_login_attempts != 0 || row.lockout_until != 0 {
        let _ = users_raw
            .update_one(
                doc! { "_id": &row.id },
                doc! { "$set": { "failed_login_attempts": 0i32, "lockout_until": 0i64 } },
            )
            .await;
    }

    let sessions = db.collection::<mongodb::bson::Document>("sessions");
    let _ = sessions
        .delete_many(doc! { "$or": [
            { "user_id": &row.id },
            { "expires_at": { "$lte": now } },
        ] })
        .await;

    let session_id = Uuid::new_v4();
    sessions
        .insert_one(doc! {
            "_id": session_id.to_string(),
            "user_id": &row.id,
            "created_at": now,
            "expires_at": now + SESSION_MAX_AGE,
        })
        .await
        .map_err(|e| {
            tracing::error!("DB: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Session creation failed")
        })?;

    tracing::info!(email = %p.email, user_id = %row.id, "user logged in");

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
        StatusCode::OK,
        headers,
        Json(UserResponse {
            id: Uuid::parse_str(&row.id).unwrap_or(Uuid::nil()),
            username: row.username,
            email: row.email,
            role: row.role,
            expires_at: now + SESSION_MAX_AGE,
        }),
    ))
}
