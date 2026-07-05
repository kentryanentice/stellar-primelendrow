use sqlx::PgExecutor;
use uuid::Uuid;

use crate::infra::storage::decode_b64;

/// 8MB decoded per image — generous for a phone photo, small enough that two
/// of them stay under the route body limit.
pub const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;

/// Mirrors the frontend's `IdType`.
pub const ID_TYPES: [&str; 5] = ["philsys", "drivers-license", "passport", "postal", "unknown"];

pub const MAX_NAME_LEN: usize = 100;
pub const MAX_DOB_LEN: usize = 32;
pub const MAX_ID_NUMBER_LEN: usize = 64;
pub const MAX_REASON_LEN: usize = 500;

/// Decode a client-supplied image (raw base64 or full `data:` URL) and sniff
/// its real type from magic bytes — the client-declared MIME type is never
/// trusted. Returns (bytes, extension, content type).
pub fn decode_image(input: &str) -> Result<(Vec<u8>, &'static str, &'static str), &'static str> {
    let b64 = input
        .rsplit_once("base64,")
        .map(|(_, rest)| rest)
        .unwrap_or(input);
    let bytes = decode_b64(b64.trim()).map_err(|_| "Invalid image encoding")?;
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err("Image too large (max 8MB)");
    }
    if bytes.len() >= 3 && bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Ok((bytes, "jpg", "image/jpeg"))
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Ok((bytes, "png", "image/png"))
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Ok((bytes, "webp", "image/webp"))
    } else {
        Err("Unsupported image format — use JPEG, PNG, or WebP")
    }
}

/// Stellar public key: 'G' + 55 base32 chars (A-Z, 2-7).
pub fn is_valid_stellar_address(addr: &str) -> bool {
    addr.len() == 56
        && addr.starts_with('G')
        && addr
            .chars()
            .all(|c| c.is_ascii_uppercase() || ('2'..='7').contains(&c))
}

/// Append a row to the audit trail. Failures are logged, never propagated —
/// an audit hiccup must not roll back or mask the action it describes (the
/// action itself already happened or is inside its own transaction).
pub async fn audit<'e, E: PgExecutor<'e>>(
    executor: E,
    submission_id: Uuid,
    user_id: Uuid,
    actor_id: Option<Uuid>,
    action: &str,
    detail: Option<&str>,
) {
    if let Err(e) = sqlx::query(
        "INSERT INTO public.kyc_audit_log (submission_id, user_id, actor_id, action, detail)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(submission_id)
    .bind(user_id)
    .bind(actor_id)
    .bind(action)
    .bind(detail)
    .execute(executor)
    .await
    {
        tracing::error!(%submission_id, action, "kyc audit insert failed: {e}");
    }
}
