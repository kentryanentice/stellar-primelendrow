use chrono::Utc;
use mongodb::{Database, bson::doc};
use std::time::Duration;

const SWEEP_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// Spawns a background task that periodically deletes expired documents from
/// the auth collections so they don't accumulate: sessions, verification
/// codes, password reset codes, unredeemed access-token reservations past
/// their expiry, and spent envelope nonces.
pub fn spawn(db: Database) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(SWEEP_INTERVAL);
        loop {
            ticker.tick().await;
            let now = Utc::now().timestamp();
            if let Err(e) = sweep(&db, now).await {
                tracing::error!("gc sweep failed: {e}");
            }
        }
    });
}

async fn sweep(db: &Database, now: i64) -> mongodb::error::Result<()> {
    let coll = |name: &str| db.collection::<mongodb::bson::Document>(name);

    // BSON range queries are type-bracketed: `$lte: now` never matches a
    // missing or null expires_at, so tokens without an expiry are safe.
    coll("sessions")
        .delete_many(doc! { "expires_at": { "$lte": now } })
        .await?;

    coll("verification_codes")
        .delete_many(doc! { "expires_at": { "$lte": now } })
        .await?;

    coll("password_reset_codes")
        .delete_many(doc! { "expires_at": { "$lte": now } })
        .await?;

    coll("access_tokens")
        .delete_many(doc! { "expires_at": { "$lte": now }, "redeemed_by": null })
        .await?;

    // used_nonces stores the envelope's ingress_expiry, which is in
    // *milliseconds* — every other expires_at in this sweep is seconds.
    coll("used_nonces")
        .delete_many(doc! { "expires_at": { "$lte": Utc::now().timestamp_millis() } })
        .await?;

    Ok(())
}
