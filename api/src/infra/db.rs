use mongodb::{
    Client, Database, IndexModel,
    bson::doc,
    options::IndexOptions,
};
use std::env;

/// Connect to MongoDB and hand back the app database. The driver connects
/// lazily, so this succeeds even if Atlas is briefly unreachable — the first
/// real operation surfaces the error, mirroring the old `connect_lazy`
/// Postgres pool.
pub async fn init_db() -> Database {
    let uri = env::var("MONGODB_URI")
        .expect("MONGODB_URI must be set")
        .trim()
        .to_string();

    let client = Client::with_uri_str(&uri)
        .await
        .expect("Invalid MONGODB_URI");

    let db_name = env::var("MONGODB_DB").unwrap_or_else(|_| "primelendrow".to_string());
    let db = client.database(&db_name);

    // Index creation replaces the SQL migrations. It is idempotent
    // (createIndexes with an identical spec is a no-op), and failure is loud
    // but non-fatal so a transient outage at boot doesn't crash-loop the
    // service; the app-level checks still guard every path the indexes
    // backstop.
    if let Err(e) = ensure_indexes(&db).await {
        tracing::error!(
            "MongoDB index creation failed — unique-index race protection is NOT in place: {e}"
        );
    }

    db
}

/// The schema, expressed as indexes.
///
/// Uniqueness that the SQL schema enforced with UNIQUE constraints is carried
/// here either by `_id` choice (verification/reset codes are keyed by email,
/// access tokens by token, nonces by nonce) or by explicit unique indexes.
/// Emails are lowercased by the app before every write and query, so a plain
/// unique index matches the old `lower(email)` index.
async fn ensure_indexes(db: &Database) -> mongodb::error::Result<()> {
    let unique = || IndexOptions::builder().unique(true).build();

    db.collection::<mongodb::bson::Document>("users")
        .create_index(
            IndexModel::builder()
                .keys(doc! { "email": 1 })
                .options(unique())
                .build(),
        )
        .await?;

    let sessions = db.collection::<mongodb::bson::Document>("sessions");
    sessions
        .create_index(IndexModel::builder().keys(doc! { "user_id": 1 }).build())
        .await?;
    sessions
        .create_index(IndexModel::builder().keys(doc! { "expires_at": 1 }).build())
        .await?;

    for coll in ["verification_codes", "password_reset_codes", "access_tokens", "used_nonces"] {
        db.collection::<mongodb::bson::Document>(coll)
            .create_index(IndexModel::builder().keys(doc! { "expires_at": 1 }).build())
            .await?;
    }

    let kyc = db.collection::<mongodb::bson::Document>("kyc_submissions");
    // One live submission per user; the same government ID can never verify
    // two accounts. Partial (a rejected attempt doesn't burn the slot), same
    // as the old partial unique indexes — the database stays the source of
    // truth under concurrent submits.
    let live = doc! { "status": { "$in": ["pending", "approved"] } };
    kyc.create_index(
        IndexModel::builder()
            .keys(doc! { "user_id": 1 })
            .options(
                IndexOptions::builder()
                    .unique(true)
                    .partial_filter_expression(live.clone())
                    .build(),
            )
            .build(),
    )
    .await?;
    kyc.create_index(
        IndexModel::builder()
            .keys(doc! { "id_number_hash": 1 })
            .options(
                IndexOptions::builder()
                    .unique(true)
                    .partial_filter_expression(live)
                    .build(),
            )
            .build(),
    )
    .await?;
    kyc.create_index(
        IndexModel::builder()
            .keys(doc! { "status": 1, "created_at": 1 })
            .build(),
    )
    .await?;
    kyc.create_index(
        IndexModel::builder()
            .keys(doc! { "user_id": 1, "created_at": -1 })
            .build(),
    )
    .await?;

    db.collection::<mongodb::bson::Document>("kyc_audit_log")
        .create_index(
            IndexModel::builder()
                .keys(doc! { "submission_id": 1, "created_at": 1 })
                .build(),
        )
        .await?;

    Ok(())
}

/// True when `e` is a duplicate-key error (code 11000) — the Mongo analogue
/// of a Postgres unique violation, and the signal that a race lost to an
/// `_id` or unique index.
pub fn is_duplicate_key(e: &mongodb::error::Error) -> bool {
    use mongodb::error::{ErrorKind, WriteFailure};
    match &*e.kind {
        ErrorKind::Write(WriteFailure::WriteError(we)) => we.code == 11000,
        ErrorKind::InsertMany(f) => f
            .write_errors
            .as_ref()
            .is_some_and(|errs| errs.iter().any(|we| we.code == 11000)),
        _ => false,
    }
}
