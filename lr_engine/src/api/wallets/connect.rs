use axum::{
    Extension, Json,
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use super::shared::{
    MAX_LABEL_LEN, MAX_WALLETS_PER_USER, audit, challenge_message, parse_address,
    verify_stellar_signature,
};
use crate::api::users::shared::{E, require_verified_user};

#[derive(Deserialize)]
pub struct ConnectInput {
    nonce: String,
    address: String,
    /// Base64 Ed25519 signature (SEP-0053) over the challenge message.
    signature: String,
    #[serde(default)]
    label: Option<String>,
}

#[derive(Serialize)]
pub struct WalletResponse {
    pub id: Uuid,
    pub address: String,
    pub label: Option<String>,
    pub source: String,
    pub status: String,
    pub connected_at: i64,
}

pub async fn connect(
    Extension(pool): Extension<PgPool>,
    headers: HeaderMap,
    Json(p): Json<ConnectInput>,
) -> Result<Json<WalletResponse>, E> {
    let user_id = require_verified_user(&pool, &headers).await?;

    let address = p.address.trim().to_string();
    let pubkey_bytes =
        parse_address(&address).map_err(|m| (StatusCode::UNPROCESSABLE_ENTITY, m))?;

    let label = match p.label.as_deref().map(str::trim) {
        Some(l) if !l.is_empty() => {
            if l.len() > MAX_LABEL_LEN {
                return Err((StatusCode::UNPROCESSABLE_ENTITY, "Label too long"));
            }
            Some(l.to_string())
        }
        _ => None,
    };

    // One-time use: the delete *is* the check, same pattern as
    // api::verified's used_nonces insert-is-the-check. A concurrent replay
    // of the same nonce loses this race and falls through to "not found".
    let now = Utc::now().timestamp();
    let expires_at: Option<i64> = sqlx::query_scalar(
        "DELETE FROM public.wallet_challenges
          WHERE nonce = $1 AND user_id = $2
          RETURNING expires_at",
    )
    .bind(&p.nonce)
    .bind(user_id)
    .fetch_optional(&pool)
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

    // Re-derive the exact message from the stored nonce/expiry — the client
    // never gets to assert what was signed.
    let message = challenge_message(&p.nonce, expires_at);
    verify_stellar_signature(&pubkey_bytes, &message, &p.signature)
        .map_err(|m| (StatusCode::UNAUTHORIZED, m))?;

    let active_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM public.wallets WHERE user_id = $1 AND status = 'active'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        tracing::error!("DB wallet count: {e}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Unable to connect wallet",
        )
    })?;
    if active_count >= MAX_WALLETS_PER_USER {
        return Err((
            StatusCode::CONFLICT,
            "Wallet limit reached — disconnect one before adding another",
        ));
    }

    // Upsert on (user_id, address): reconnecting an address the caller has
    // used before (and just re-proved ownership of) flips it back to active
    // instead of forking a duplicate row. `source` is deliberately left out
    // of the SET list, so a kyc_verified anchor row stays kyc_verified
    // through a reconnect. A conflict against the *other* unique index
    // (idx_wallets_address_active — someone else's account already holds
    // this address active) isn't this statement's target, so it surfaces
    // as a real unique-violation error instead of silently upserting.
    let row: Result<(Uuid, String, Option<String>, String, String, i64), sqlx::Error> =
        sqlx::query_as(
            "INSERT INTO public.wallets
                (user_id, address, label, source, status, connected_at, created_at, updated_at)
             VALUES ($1, $2, $3, 'user_added', 'active', $4, $4, $4)
             ON CONFLICT (user_id, address) DO UPDATE
                SET status = 'active',
                    disconnected_at = NULL,
                    label = COALESCE(EXCLUDED.label, wallets.label),
                    connected_at = EXCLUDED.connected_at,
                    updated_at = EXCLUDED.updated_at
             RETURNING id, address, label, source, status, connected_at",
        )
        .bind(user_id)
        .bind(&address)
        .bind(&label)
        .bind(now)
        .fetch_one(&pool)
        .await;

    let (id, address, label, source, status, connected_at) = match row {
        Ok(row) => row,
        Err(e) => {
            if e.as_database_error()
                .is_some_and(|d| d.is_unique_violation())
            {
                return Err((
                    StatusCode::CONFLICT,
                    "This wallet is already connected to a different account",
                ));
            }
            tracing::error!("DB wallet connect: {e}");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Unable to connect wallet",
            ));
        }
    };

    audit(&pool, id, user_id, &address, "connected").await;
    tracing::info!(%user_id, wallet_id = %id, "wallet connected");

    Ok(Json(WalletResponse {
        id,
        address,
        label,
        source,
        status,
        connected_at,
    }))
}
