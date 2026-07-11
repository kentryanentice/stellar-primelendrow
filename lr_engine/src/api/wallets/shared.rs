use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};
use sqlx::PgExecutor;
use uuid::Uuid;

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
/// and CRC16-XMODEM checksum, via the SDF-maintained `stellar-strkey` crate
/// (stronger than kyc::shared::is_valid_stellar_address's shape-only check,
/// which is fine for its own use but not what a signature-ownership proof
/// should be checked against). Returns the raw 32-byte ed25519 public key.
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
