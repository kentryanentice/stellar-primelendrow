use axum::http::StatusCode;
use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::Utc;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};
use sqlx::{PgExecutor, PgPool};
use uuid::Uuid;

use crate::api::users::shared::E;

pub const MAX_LABEL_LEN: usize = 50;
/// A bookkeeping limit, not a security boundary — easy to raise if it turns
/// out too tight.
pub const MAX_WALLETS_PER_USER: i64 = 5;
/// Long enough to connect a wallet extension and approve the signature
/// prompt, short enough that a stale challenge is worthless if it leaks.
pub const CHALLENGE_TTL_SECS: i64 = 5 * 60;

/// The exact message a wallet is asked to sign to prove control of its
/// address. Re-derived server-side from the stored nonce + expiry on verify
/// — never trusted from the client — so this only needs to be deterministic,
/// not secret.
pub fn challenge_message(nonce: &str, expires_at: i64) -> String {
    format!(
        "PrimeLendRow wallet verification\n\
         Nonce: {nonce}\n\
         Expires: {expires_at}\n\
         This request will not move funds or sign any transaction."
    )
}

/// Decodes and validates a Stellar `G...` address — base32, version byte,
/// and CRC16-XMODEM checksum, via the SDF-maintained `stellar-strkey` crate,
/// so a mistyped address is refused rather than merely looking right.
/// Returns the raw 32-byte ed25519 public key.
pub fn parse_address(address: &str) -> Result<[u8; 32], &'static str> {
    address
        .parse::<stellar_strkey::ed25519::PublicKey>()
        .map(|pk| pk.0)
        .map_err(|_| "Invalid wallet address")
}

/// Verifies a SEP-0053 message signature: the signer must hold the private
/// key behind `pubkey_bytes` to have produced `signature_b64` over `message`.
///
/// SEP-0053: signature = Ed25519_Sign(privkey, SHA256("Stellar Signed Message:\n" + message)).
pub fn verify_stellar_signature(
    pubkey_bytes: &[u8; 32],
    message: &str,
    signature_b64: &str,
) -> Result<(), &'static str> {
    let key = VerifyingKey::from_bytes(pubkey_bytes).map_err(|_| "Invalid wallet address")?;

    let sig_bytes = STANDARD
        .decode(signature_b64)
        .map_err(|_| "Invalid signature encoding")?;
    let sig_arr: [u8; 64] = sig_bytes
        .as_slice()
        .try_into()
        .map_err(|_| "Invalid signature length")?;
    let sig = Signature::from_bytes(&sig_arr);

    let mut payload = Vec::with_capacity(25 + message.len());
    payload.extend_from_slice(b"Stellar Signed Message:\n");
    payload.extend_from_slice(message.as_bytes());
    let hash = Sha256::digest(&payload);

    key.verify(hash.as_slice(), &sig)
        .map_err(|_| "Wallet verification failed")
}

/// The whole ownership proof: uses up the one-time challenge `nonce` issued
/// to `user_id` by api::wallets::challenge, and checks `signature_b64` is the
/// key behind `pubkey_bytes` signing that challenge's message. Shared by
/// connecting a wallet (api::wallets::connect) and submitting KYC, so both
/// accept exactly the same proof.
///
/// One-time use: the delete *is* the check, same pattern as
/// api::verified's used_nonces insert-is-the-check. A concurrent replay of
/// the same nonce loses this race and falls through to "not found". The
/// message is re-derived from the stored nonce/expiry — the client never
/// gets to assert what was signed.
pub async fn redeem_challenge(
    pool: &PgPool,
    user_id: Uuid,
    nonce: &str,
    pubkey_bytes: &[u8; 32],
    signature_b64: &str,
) -> Result<(), E> {
    let now = Utc::now().timestamp();
    let expires_at: Option<i64> = sqlx::query_scalar(
        "DELETE FROM public.wallet_challenges
          WHERE nonce = $1 AND user_id = $2
          RETURNING expires_at",
    )
    .bind(nonce)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("DB wallet challenge redeem: {e}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Wallet verification failed",
        )
    })?;

    let expires_at = expires_at.ok_or((
        StatusCode::BAD_REQUEST,
        "Verification challenge expired or invalid — try connecting again",
    ))?;
    if expires_at < now {
        return Err((
            StatusCode::BAD_REQUEST,
            "Verification challenge expired or invalid — try connecting again",
        ));
    }

    let message = challenge_message(nonce, expires_at);
    verify_stellar_signature(pubkey_bytes, &message, signature_b64)
        .map_err(|m| (StatusCode::UNAUTHORIZED, m))
}

/// Append a row to the audit trail. Failures are logged, never propagated —
/// same rationale as kyc::shared::audit: an audit hiccup must not roll back
/// or mask the action it describes.
pub async fn audit<'e, E: PgExecutor<'e>>(
    executor: E,
    wallet_id: Uuid,
    user_id: Uuid,
    address: &str,
    action: &str,
) {
    if let Err(e) = sqlx::query(
        "INSERT INTO public.wallet_audit_log (wallet_id, user_id, address, action)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(wallet_id)
    .bind(user_id)
    .bind(address)
    .bind(action)
    .execute(executor)
    .await
    {
        tracing::error!(%wallet_id, action, "wallet audit insert failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_that_only_looks_right_is_refused() {
        let good = String::from(stellar_strkey::ed25519::PublicKey([7; 32]).to_string().as_str());
        assert_eq!(parse_address(&good), Ok([7; 32]));

        // Same shape — 56 characters, a leading G, all base32 — with one
        // character changed, which only the checksum can catch.
        let mut bad = good.into_bytes();
        bad[20] = if bad[20] == b'A' { b'B' } else { b'A' };
        let bad = String::from_utf8(bad).unwrap();
        assert!(parse_address(&bad).is_err());
    }
}
