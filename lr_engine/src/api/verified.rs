use axum::{
    Json,
    body::Body,
    extract::FromRequest,
    http::{Request, StatusCode},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::Utc;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::Deserialize;
use sqlx::PgPool;

const MAX_WINDOW_MS: i64 = 2 * 60 * 1000;
const CLOCK_SKEW_MS: i64 = 5 * 1000;
/// Honest clients send a UUID (36 chars); anything much longer is junk that
/// would only bloat the used_nonces table.
const MAX_NONCE_LEN: usize = 128;

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
    let pool = req
        .extensions()
        .get::<PgPool>()
        .cloned()
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Missing database"))?;

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

    if meta.nonce.is_empty() || meta.nonce.len() > MAX_NONCE_LEN {
        return Err((StatusCode::BAD_REQUEST, "Invalid nonce"));
    }

    // DB-backed so replay protection holds across instances and restarts —
    // the old in-memory map only ever saw one Cloud Run instance's traffic.
    // The insert is the check: a conflict means the nonce was already spent.
    // Expired rows are swept by infra::gc (expires_at is in *milliseconds*,
    // straight from the signed envelope).
    let inserted = sqlx::query(
        "INSERT INTO public.used_nonces (nonce, expires_at)
         VALUES ($1, $2) ON CONFLICT (nonce) DO NOTHING",
    )
    .bind(&meta.nonce)
    .bind(meta.ingress_expiry)
    .execute(&pool)
    .await
    .map_err(|e| {
        tracing::error!("DB nonce: {e}");
        (StatusCode::INTERNAL_SERVER_ERROR, "Verification failed")
    })?;
    if inserted.rows_affected() == 0 {
        return Err((StatusCode::BAD_REQUEST, "Nonce already used"));
    }

    Ok((msg, pubkey_bytes))
}

impl<S: Send + Sync> FromRequest<S> for Verified {
    type Rejection = (StatusCode, &'static str);

    async fn from_request(req: Request<Body>, state: &S) -> Result<Self, Self::Rejection> {
        let (msg, pubkey) = extract_verified(req, state).await?;
        Ok(Verified(msg, pubkey))
    }
}
