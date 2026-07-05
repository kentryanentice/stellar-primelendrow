use chrono::Utc;
use sqlx::PgPool;
use std::time::Duration;

const SWEEP_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// Spawns a background task that periodically deletes expired rows from the
/// auth tables so they don't accumulate: sessions, verification codes, password
/// reset codes, unredeemed access-token reservations past their expiry, and
/// spent envelope nonces.
pub fn spawn(pool: PgPool) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(SWEEP_INTERVAL);
        loop {
            ticker.tick().await;
            let now = Utc::now().timestamp();
            if let Err(e) = sweep(&pool, now).await {
                tracing::error!("gc sweep failed: {e}");
            }
        }
    });
}

async fn sweep(pool: &PgPool, now: i64) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM public.sessions WHERE expires_at <= $1", now)
        .execute(pool)
        .await?;

    sqlx::query!(
        "DELETE FROM public.verification_codes WHERE expires_at <= $1",
        now
    )
    .execute(pool)
    .await?;

    sqlx::query!(
        "DELETE FROM public.password_reset_codes WHERE expires_at <= $1",
        now
    )
    .execute(pool)
    .await?;

    sqlx::query!(
        "DELETE FROM public.access_tokens
         WHERE expires_at IS NOT NULL AND expires_at <= $1 AND redeemed_by IS NULL",
        now
    )
    .execute(pool)
    .await?;

    // used_nonces stores the envelope's ingress_expiry, which is in
    // *milliseconds* — every other expires_at in this sweep is seconds.
    sqlx::query("DELETE FROM public.used_nonces WHERE expires_at <= $1")
        .bind(Utc::now().timestamp_millis())
        .execute(pool)
        .await?;

    Ok(())
}
