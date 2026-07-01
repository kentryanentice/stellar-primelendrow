use axum::{
    Json,
    body::Body,
    extract::FromRequest,
    http::{Request, StatusCode},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::Utc;
use dashmap::DashMap;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::Deserialize;
use std::sync::Arc;

pub type NonceStore = Arc<DashMap<String, i64>>;

const MAX_WINDOW_MS: i64 = 2 * 60 * 1000;
const CLOCK_SKEW_MS: i64 = 5 * 1000;
const MAX_NONCE_STORE: usize = 100_000;

/// Accepts any valid Ed25519 signature. Used for /register, /verify, /login.
pub struct Verified(pub Vec<u8>, pub Vec<u8>);

#[derive(Deserialize)]
struct Envelope {
    payload: String,
    pubkey: String,
    signature: String,
}

#[derive(Deserialize)]
struct Meta {
    nonce: String,
    ingress_expiry: i64,
}

/// Validates the signed envelope (signature, nonce, expiry).
/// Returns (payload_bytes, pubkey_bytes).
async fn extract_verified<S: Send + Sync>(
    req: Request<Body>,
    state: &S,
) -> Result<(Vec<u8>, Vec<u8>), (StatusCode, &'static str)> {
    let nonces = req
        .extensions()
        .get::<NonceStore>()
        .cloned()
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Missing nonce store"))?;

    let Json(env): Json<Envelope> = Json::from_request(req, state)
        .await
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid JSON"))?;

    let msg = STANDARD
        .decode(&env.payload)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid payload"))?;

    let pubkey_bytes = STANDARD
        .decode(&env.pubkey)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid pubkey"))?;

    let pubkey_arr: [u8; 32] = pubkey_bytes
        .as_slice()
        .try_into()
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid pubkey length"))?;

    let key = VerifyingKey::from_bytes(&pubkey_arr)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid public key"))?;

    let sig_bytes = STANDARD
        .decode(&env.signature)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid signature"))?;

    let sig = Signature::from_bytes(
        sig_bytes
            .as_slice()
            .try_into()
            .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid signature length"))?,
    );

    key.verify(&msg, &sig)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid signature"))?;

    let meta: Meta = serde_json::from_slice(&msg)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Missing nonce or expiry"))?;

    let now_ms = Utc::now().timestamp_millis();

    if meta.ingress_expiry < now_ms {
        return Err((StatusCode::BAD_REQUEST, "Request expired"));
    }
    if meta.ingress_expiry > now_ms + MAX_WINDOW_MS + CLOCK_SKEW_MS {
        return Err((StatusCode::BAD_REQUEST, "Expiry too far in future"));
    }

    if nonces.len() >= MAX_NONCE_STORE {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            "Server busy, try again later",
        ));
    }

    match nonces.entry(meta.nonce) {
        dashmap::mapref::entry::Entry::Occupied(_) => {
            return Err((StatusCode::BAD_REQUEST, "Nonce already used"));
        }
        dashmap::mapref::entry::Entry::Vacant(e) => {
            e.insert(meta.ingress_expiry);
        }
    }
    nonces.retain(|_, exp| *exp > now_ms);

    Ok((msg, pubkey_bytes))
}

impl<S: Send + Sync> FromRequest<S> for Verified {
    type Rejection = (StatusCode, &'static str);

    async fn from_request(req: Request<Body>, state: &S) -> Result<Self, Self::Rejection> {
        let (msg, pubkey) = extract_verified(req, state).await?;
        Ok(Verified(msg, pubkey))
    }
}
