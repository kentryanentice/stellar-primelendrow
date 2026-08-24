use axum::{
    extract::ConnectInfo,
    http::{HeaderMap, Request, StatusCode},
    middleware::Next,
    response::Response,
};
use dashmap::DashMap;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use subtle::ConstantTimeEq;
use tokio::sync::Mutex;
use tokio::time::Instant;

type HmacSha256 = Hmac<Sha256>;

/// Bound-then-sweep cap on the per-key bucket map, same pattern as
/// `login_guard` â€” a flood of unique IPs can't grow the map without limit,
/// and idle buckets (which are full anyway) are dropped once stale.
const MAX_KEYS: usize = 100_000;

#[derive(Clone)]
pub struct RateLimiter {
    per_key: Arc<DashMap<String, TokenBucket>>,
    global: Arc<Mutex<TokenBucket>>,
    per_key_max: u32,
    window: Duration,
    device_secret: Arc<Vec<u8>>,
}

#[derive(Debug)]
struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
    capacity: u32,
    refill_rate: f64,
}

impl TokenBucket {
    fn new(capacity: u32, window: Duration) -> Self {
        let refill_rate = capacity as f64 / window.as_secs_f64();
        Self {
            tokens: capacity as f64,
            last_refill: Instant::now(),
            capacity,
            refill_rate,
        }
    }

    fn try_acquire(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();

        let tokens_to_add = elapsed * self.refill_rate;
        self.tokens = (self.tokens + tokens_to_add).min(self.capacity as f64);
        self.last_refill = now;

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

impl RateLimiter {
    /// `global_max` must be a large multiple of `per_key_max`: when the two
    /// were equal, a single IP could drain the entire global bucket and 429
    /// every other client.
    pub fn new(global_max: u32, per_key_max: u32, window: Duration, device_secret: String) -> Self {
        Self {
            per_key: Arc::new(DashMap::new()),
            global: Arc::new(Mutex::new(TokenBucket::new(global_max, window))),
            per_key_max,
            window,
            device_secret: Arc::new(device_secret.into_bytes()),
        }
    }

    async fn check_bucket(&self, key: String) -> bool {
        if self.per_key.len() >= MAX_KEYS {
            let now = Instant::now();
            let window = self.window;
            self.per_key
                .retain(|_, b| now.duration_since(b.last_refill) < window);
        }
        self.per_key
            .entry(key)
            .or_insert_with(|| TokenBucket::new(self.per_key_max, self.window))
            .try_acquire()
    }

    pub async fn check(&self, device_id: Option<String>, ip: IpAddr) -> bool {
        // Global limit
        {
            let mut global = self.global.lock().await;
            if !global.try_acquire() {
                return false;
            }
        }

        // Per IP
        if !self.check_bucket(format!("ip:{ip}")).await {
            return false;
        }

        // Per Device (if valid)
        if let Some(device) = device_id
            && !self.check_bucket(format!("device:{device}")).await
        {
            return false;
        }

        true
    }

    fn verify_device_id(&self, raw: &str) -> Option<String> {
        let parts: Vec<&str> = raw.split('.').collect();
        if parts.len() != 2 {
            return None;
        }

        let uuid = parts[0];
        let signature = parts[1];

        let mut mac = HmacSha256::new_from_slice(&self.device_secret).ok()?;
        mac.update(uuid.as_bytes());

        let expected = hex::encode(mac.finalize().into_bytes());

        if expected.as_bytes().ct_eq(signature.as_bytes()).into() {
            Some(uuid.to_string())
        } else {
            None
        }
    }
}

/// How many proxy hops in front of us append to X-Forwarded-For. 0 (default)
/// means "no trusted proxy": only a localhost peer may speak for the client
/// (local dev with a dev proxy). Behind Cloud Run / a load balancer the peer
/// address is always the platform's front end, so per-IP limits would collapse
/// into one shared bucket for every user â€” set TRUST_PROXY_HOPS=1 there so the
/// client IP is taken from X-Forwarded-For instead. Entries beyond the trusted
/// hop count are client-supplied and must never be believed.
fn trusted_proxy_hops() -> usize {
    static HOPS: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *HOPS.get_or_init(|| {
        std::env::var("TRUST_PROXY_HOPS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    })
}

pub fn extract_ip(
    headers: &HeaderMap,
    connect_info: Option<&ConnectInfo<SocketAddr>>,
) -> Option<IpAddr> {
    let peer_ip = connect_info.map(|ci| ci.0.ip())?;

    let hops = trusted_proxy_hops();
    if hops > 0 {
        // The rightmost `hops` XFF entries were appended by proxies we trust;
        // the entry just before them is the real client.
        if let Some(forwarded) = headers.get("x-forwarded-for")
            && let Ok(forwarded) = forwarded.to_str()
        {
            let entries: Vec<&str> = forwarded.split(',').map(str::trim).collect();
            if entries.len() >= hops
                && let Ok(ip) = entries[entries.len() - hops].parse::<IpAddr>()
            {
                return Some(ip);
            }
        }
        return Some(peer_ip);
    }

    let trusted_proxies: &[IpAddr] = &["127.0.0.1".parse().unwrap(), "::1".parse().unwrap()];

    if trusted_proxies.contains(&peer_ip)
        && let Some(forwarded) = headers.get("x-forwarded-for")
        && let Ok(forwarded) = forwarded.to_str()
        && let Some(ip_str) = forwarded.split(',').next()
        && let Ok(ip) = ip_str.trim().parse::<IpAddr>()
    {
        return Some(ip);
    }

    Some(peer_ip)
}

fn extract_device(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-device-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
}

pub async fn enforce_rate_limit(
    limiter: RateLimiter,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let headers = req.headers();
    let connect_info = req.extensions().get::<ConnectInfo<SocketAddr>>();

    let ip = extract_ip(headers, connect_info).ok_or(StatusCode::FORBIDDEN)?;

    let device_raw = extract_device(headers);

    let device_valid = device_raw
        .as_deref()
        .and_then(|d| limiter.verify_device_id(d));

    if limiter.check(device_valid, ip).await {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::TOO_MANY_REQUESTS)
    }
}
