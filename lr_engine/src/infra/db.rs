use sqlx::{PgPool, postgres::PgPoolOptions};
use std::env;
use std::time::Duration;

pub async fn init_db_pool() -> PgPool {
    let db_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set")
        .trim()
        .to_string();

    PgPoolOptions::new()
        .max_connections(5)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(10))
        // .idle_timeout(Duration::from_secs(30))
        // .max_lifetime(Duration::from_secs(300))
        .connect_lazy(&db_url)
        .expect("Failed to connect to PostgreSQL via pooler")
}
