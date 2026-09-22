use sqlx::{PgPool, postgres::PgPoolOptions};
use std::env;
use std::time::Duration;

pub async fn init_db_pool() -> PgPool {
    let db_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set")
        .trim()
        .to_string();

    // Sized against Supabase's Nano tier: the pooler allows 200 client
    // connections (fixed) but only 15 server connections to Postgres per
    // user+db, shared across every pooler. 15 is the number that binds, and it
    // is shared with psql sessions, the dashboard SQL editor and migrations —
    // so 10 here leaves 5 for a human.
    //
    // 5 was too few. Four background sweeps run on their own clocks (payouts
    // and capture recovery every 60s, gc every 5 min, term-end scores every 15
    // min), and `tokio::time::interval` fires immediately on its first tick, so
    // all four contend at boot and again whenever their periods coincide. With
    // five slots that left one for request traffic, and the sweeps timed out
    // waiting on each other.
    PgPoolOptions::new()
        .max_connections(10)
        // Pre-warmed rather than cold. The engine talks to a pooler in
        // ap-southeast-1, so opening a connection costs a TCP and TLS round
        // trip — the 2s, 2.6s and 7.3s "slow acquire" warnings were that
        // handshake, not contention over busy connections. Keeping three open
        // means a sweep waking up usually finds one ready.
        .min_connections(2)
        .acquire_timeout(Duration::from_secs(10))
        // Deliberately no `idle_timeout`. Over a link this long, closing idle
        // connections only to re-handshake them costs more than holding them,
        // and the 30s value tried here before would have closed them between
        // almost every sweep.
        //
        // `max_lifetime` is kept, but at 30 minutes rather than the 5 tried
        // before: long enough not to re-handshake constantly, short enough that
        // a connection the pooler has quietly dropped gets retired rather than
        // handed out dead.
        .max_lifetime(Duration::from_secs(30 * 60))
        .connect_lazy(&db_url)
        .expect("Failed to connect to PostgreSQL via pooler")
}
