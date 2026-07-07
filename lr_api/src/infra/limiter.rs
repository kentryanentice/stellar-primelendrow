use axum::extract::ConnectInfo;
use axum::{
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use dashmap::DashMap;
use std::{net::IpAddr, sync::Arc};
use tokio::sync::Semaphore;

/// Bound-then-sweep cap, same pattern as the rate limiter and login guard —
/// without it, one semaphore per unique client IP is retained forever, and an
/// IPv6 rotation flood grows the map without limit.
const MAX_KEYS: usize = 100_000;

#[derive(Clone)]
pub struct ConcurrencyLimiter {
    inner: Arc<DashMap<IpAddr, Arc<Semaphore>>>,
    max_per_ip: usize,
}

impl ConcurrencyLimiter {
    pub fn new(max_per_ip: usize) -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
            max_per_ip,
        }
    }

    fn get_semaphore(&self, ip: IpAddr) -> Arc<Semaphore> {
        if self.inner.len() >= MAX_KEYS {
            // an idle IP's semaphore has every permit free; in-flight requests
            // hold their Arc regardless, so dropping idle entries is safe
            let max = self.max_per_ip;
            self.inner.retain(|_, sem| sem.available_permits() < max);
        }
        self.inner
            .entry(ip)
            .or_insert_with(|| Arc::new(Semaphore::new(self.max_per_ip)))
            .clone()
    }
}

pub async fn enforce_concurrency(
    limiter: ConcurrencyLimiter,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Same proxy-aware client IP as the rate limiter — keyed on the raw peer
    // address this would collapse into a single shared semaphore behind
    // Cloud Run's front end.
    let ip = super::rate::extract_ip(
        req.headers(),
        req.extensions()
            .get::<ConnectInfo<std::net::SocketAddr>>(),
    );

    if let Some(ip) = ip {
        let semaphore = limiter.get_semaphore(ip);
        match semaphore.try_acquire() {
            Ok(permit) => {
                let res = next.run(req).await;
                drop(permit);
                Ok(res)
            }
            Err(_) => Err(StatusCode::TOO_MANY_REQUESTS),
        }
    } else {
        Ok(next.run(req).await)
    }
}
