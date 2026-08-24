use dashmap::DashMap;
use std::net::IpAddr;
use std::sync::OnceLock;

/// Per-(IP, email) login failure tracking.
///
/// The DB-backed per-email lockout in `login.rs` locks the *account*, which
/// lets anyone who knows a victim's email lock them out at will. This guard
/// carries the primary brute-force defense instead: 5 failures from one IP
/// against one email lock that IP+email pair for 15 minutes, so an attacker's
/// failures never block the account owner logging in from their own address.
/// The per-email DB lockout stays as a high-threshold backstop against
/// distributed (many-IP) attacks.
///
/// In-memory, like the rate limiter: per Cloud Run instance and cleared on
/// restart, which is acceptable because the global+per-IP rate limits cap how
/// fast anyone can retry across instances.
const MAX_FAILS: u32 = 5;
const LOCKOUT_SECONDS: i64 = 15 * 60;
/// Same bound-then-sweep pattern as the nonce store — keeps a flood of
/// unique (ip, email) probes from growing the map without limit.
const MAX_ENTRIES: usize = 100_000;

struct Entry {
    fails: u32,
    locked_until: i64,
    updated_at: i64,
}

fn map() -> &'static DashMap<(IpAddr, String), Entry> {
    static MAP: OnceLock<DashMap<(IpAddr, String), Entry>> = OnceLock::new();
    MAP.get_or_init(DashMap::new)
}

pub fn is_locked(ip: IpAddr, email: &str, now: i64) -> bool {
    map()
        .get(&(ip, email.to_string()))
        .is_some_and(|e| e.locked_until > now)
}

pub fn record_failure(ip: IpAddr, email: &str, now: i64) {
    let m = map();
    if m.len() >= MAX_ENTRIES {
        m.retain(|_, e| e.updated_at > now - LOCKOUT_SECONDS);
    }
    let mut e = m.entry((ip, email.to_string())).or_insert(Entry {
        fails: 0,
        locked_until: 0,
        updated_at: now,
    });
    // failures older than a lockout window don't count toward a new lock
    if e.updated_at <= now - LOCKOUT_SECONDS {
        e.fails = 0;
        e.locked_until = 0;
    }
    e.fails += 1;
    e.updated_at = now;
    if e.fails >= MAX_FAILS {
        e.locked_until = now + LOCKOUT_SECONDS;
    }
}

pub fn clear(ip: IpAddr, email: &str) {
    map().remove(&(ip, email.to_string()));
}
