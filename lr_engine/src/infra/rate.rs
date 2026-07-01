// use axum::{
//     extract::ConnectInfo,
//     http::{HeaderMap, Request, StatusCode},
//     middleware::Next,
//     response::Response,
// };
// use dashmap::DashMap;
// use std::{
//     net::{IpAddr, SocketAddr},
//     sync::Arc,
//     time::Duration,
// };
// use tokio::sync::Mutex;
// use tokio::time::Instant;

// #[derive(Clone)]
// pub struct RateLimiter {
//     inner: Arc<DashMap<IpAddr, TokenBucket>>,
//     global: Arc<Mutex<TokenBucket>>,
//     max_requests: u32,
//     window: Duration,
// }

// #[derive(Debug)]
// struct TokenBucket {
//     tokens: f64,
//     last_refill: Instant,
//     capacity: u32,
//     refill_rate: f64,
// }

// impl TokenBucket {
//     fn new(capacity: u32, window: Duration) -> Self {
//         let refill_rate = capacity as f64 / window.as_secs_f64();
//         Self {
//             tokens: capacity as f64,
//             last_refill: Instant::now(),
//             capacity,
//             refill_rate,
//         }
//     }

//     fn try_acquire(&mut self) -> bool {
//         let now = Instant::now();
//         let elapsed = now.duration_since(self.last_refill).as_secs_f64();

//         let tokens_to_add = elapsed * self.refill_rate;
//         self.tokens = (self.tokens + tokens_to_add).min(self.capacity as f64);
//         self.last_refill = now;

//         if self.tokens >= 1.0 {
//             self.tokens -= 1.0;
//             true
//         } else {
//             false
//         }
//     }
// }

// impl RateLimiter {
//     pub fn new(max_requests: u32, window: Duration) -> Self {
//         Self {
//             inner: Arc::new(DashMap::new()),
//             global: Arc::new(Mutex::new(TokenBucket::new(max_requests, window))),
//             max_requests,
//             window,
//         }
//     }

//     pub async fn check(&self, ip: IpAddr) -> bool {

//         {
//             let mut global = self.global.lock().await;
//             if !global.try_acquire() {
//                 return false;
//             }
//         }

//         self.inner
//             .entry(ip)
//             .or_insert_with(|| TokenBucket::new(self.max_requests, self.window))
//             .try_acquire()
//     }
// }

// // fn extract_ip(headers: &HeaderMap, connect_info: Option<&ConnectInfo<SocketAddr>>) -> Option<IpAddr> {
// //     let peer_ip = connect_info.map(|ci| ci.0.ip())?;

// //     let trusted_proxies: &[IpAddr] = &["127.0.0.1".parse().unwrap(), "::1".parse().unwrap()];

// //     if trusted_proxies.contains(&peer_ip) {
// //         if let Some(forwarded) = headers.get("x-forwarded-for") {
// //             if let Ok(forwarded) = forwarded.to_str() {
// //                 if let Some(ip_str) = forwarded.split(',').next() {
// //                     if let Ok(ip) = ip_str.trim().parse::<IpAddr>() {
// //                         match ip {
// //                             IpAddr::V4(v4) if v4.is_private() => return None,
// //                             IpAddr::V6(v6) if v6.is_unique_local() => return None,
// //                             _ => return Some(ip),
// //                         }
// //                     }
// //                 }
// //             }
// //         }
// //     }

// //     Some(peer_ip)
// // }

// fn extract_ip(headers: &HeaderMap, connect_info: Option<&ConnectInfo<SocketAddr>>) -> Option<IpAddr> {

//     let peer_ip = connect_info.map(|ci| ci.0.ip())?;

//     let trusted_proxies: &[IpAddr] = &["127.0.0.1".parse().unwrap(), "::1".parse().unwrap()];

//     if trusted_proxies.contains(&peer_ip) {
//         if let Some(forwarded) = headers.get("x-forwarded-for") {
//             if let Ok(forwarded) = forwarded.to_str() {
//                 if let Some(ip_str) = forwarded.split(',').next() {
//                     if let Ok(ip) = ip_str.trim().parse::<IpAddr>() {
//                         return Some(ip);
//                     }
//                 }
//             }
//         }
//     }
//     Some(peer_ip)
// }

// pub async fn enforce_rate_limit(
//     limiter: RateLimiter,
//     req: Request<axum::body::Body>,
//     next: Next,
// ) -> Result<Response, StatusCode> {
//     let headers = req.headers();
//     let connect_info = req.extensions().get::<ConnectInfo<SocketAddr>>();

//     let ip = extract_ip(headers, connect_info);

//     eprintln!("Rate limit check for IP: {ip:?}");

//     if let Some(ip) = ip {
//         if limiter.check(ip).await {
//             Ok(next.run(req).await)
//         } else {
//             Err(StatusCode::TOO_MANY_REQUESTS)
//         }
//     } else {
//         Err(StatusCode::FORBIDDEN)
//     }
// }
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

#[derive(Clone)]
pub struct RateLimiter {
    per_key: Arc<DashMap<String, TokenBucket>>,
    global: Arc<Mutex<TokenBucket>>,
    max_requests: u32,
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
    pub fn new(max_requests: u32, window: Duration, device_secret: String) -> Self {
        Self {
            per_key: Arc::new(DashMap::new()),
            global: Arc::new(Mutex::new(TokenBucket::new(max_requests, window))),
            max_requests,
            window,
            device_secret: Arc::new(device_secret.into_bytes()),
        }
    }

    async fn check_bucket(&self, key: String) -> bool {
        self.per_key
            .entry(key)
            .or_insert_with(|| TokenBucket::new(self.max_requests, self.window))
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

fn extract_ip(
    headers: &HeaderMap,
    connect_info: Option<&ConnectInfo<SocketAddr>>,
) -> Option<IpAddr> {
    let peer_ip = connect_info.map(|ci| ci.0.ip())?;

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
