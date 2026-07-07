pub mod api;
pub mod infra;
mod routes;

use std::{env, time::Duration};

use axum::http::HeaderValue;
use axum::{
    extract::Extension,
    http::Method,
    middleware::{self},
};
use dotenvy::dotenv;
use infra::csrf::enforce_csrf;
use infra::db::init_db;
use infra::limiter::{ConcurrencyLimiter, enforce_concurrency};
use infra::rate::{RateLimiter, enforce_rate_limit};
use routes::api_routes;
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenv().ok();

    let limiter = ConcurrencyLimiter::new(20);

    let device_secret = env::var("DEVICE_SECRET").unwrap_or_default();
    if device_secret.is_empty() {
        tracing::warn!(
            "DEVICE_SECRET is not set; device-id rate-limit signatures cannot be trusted"
        );
    }
    // Global capacity must dwarf the per-IP capacity: with the two equal, one
    // IP could drain the global bucket and 429 every other client.
    let rate_limiter = RateLimiter::new(10_000, 1000, Duration::from_secs(60), device_secret.clone());

    // Second, much tighter limiter for the two endpoints that trigger emails
    // (/auth/register, /auth/password-reset/request). The 60s per-address
    // cooldown caps sends per *victim*; this caps how many *distinct* victim
    // addresses one IP can target — under the general 1000/min limit a single
    // IP could burn mail quota on ~1000 strangers a minute. Same invariant as
    // above: global (300) dwarfs per-IP (5).
    let mail_rate_limiter = RateLimiter::new(300, 5, Duration::from_secs(60), device_secret);

    if env::var("OTP_HASH_SECRET").map(|v| v.is_empty()).unwrap_or(true) {
        tracing::warn!(
            "OTP_HASH_SECRET is not set; verification codes are hashed with an empty key"
        );
    }

    if env::var("WORKER_SECRET").map(|v| v.is_empty()).unwrap_or(true) {
        tracing::warn!("WORKER_SECRET is not set; the mailer worker will reject all email sends");
    }

    // KYC fails closed rather than degrading: without the encryption key or a
    // storage target, /kyc/submit returns 503 instead of storing PII in the
    // clear or documents nowhere.
    if !infra::crypto::is_configured() {
        tracing::warn!(
            "KYC_ENC_KEY is not set or invalid (expects 64 hex chars); KYC submissions will be refused"
        );
    }
    if env::var("KYC_HASH_SECRET").map(|v| v.is_empty()).unwrap_or(true) {
        tracing::warn!(
            "KYC_HASH_SECRET is not set; ID-number blind indexes are keyed with an empty secret"
        );
    }

    // Cloud Run always sets K_SERVICE, and there the TCP peer is the platform
    // front end — without the proxy-hop config every per-IP protection keys on
    // that one shared address (rate limits collapse, and the per-(IP, email)
    // login guard turns back into an account-lockout DoS). Refuse to start
    // rather than run looking protected while not being so.
    if env::var("K_SERVICE").is_ok() && env::var("TRUST_PROXY_HOPS").is_err() {
        panic!(
            "Running on Cloud Run without TRUST_PROXY_HOPS — per-IP rate limits and the login guard would key on the load balancer's address. Set TRUST_PROXY_HOPS=1."
        );
    }

    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());

    let client_url_raw =
        env::var("CLIENT_URL").unwrap_or_else(|_| "http://localhost:5173".to_string());

    let origins: Vec<HeaderValue> = client_url_raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            HeaderValue::from_str(s).unwrap_or_else(|_| panic!("Invalid CLIENT_URL entry: {s}"))
        })
        .collect();

    if origins.is_empty() {
        panic!("CLIENT_URL must contain at least one origin");
    }

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::ACCEPT,
            axum::http::header::HeaderName::from_static("x-csrf-token"),
            axum::http::header::HeaderName::from_static("x-client-name"),
            axum::http::header::HeaderName::from_static("x-client-version"),
        ])
        .expose_headers([axum::http::header::HeaderName::from_static("x-csrf-token")])
        .allow_credentials(true);

    let db = init_db().await;

    // KYC documents live in MongoDB (kyc_files) and are read back through
    // short-lived HMAC-signed URLs served by this API. The signing key is the
    // KYC hash secret (domain-separated inside MongoStorage); the base URL is
    // what the admin frontend can actually reach.
    let public_base = env::var("PUBLIC_API_URL")
        .unwrap_or_else(|_| format!("http://localhost:{port}"));
    let kyc_storage = infra::storage::MongoStorage::new(
        db.clone(),
        env::var("KYC_HASH_SECRET").unwrap_or_default().into_bytes(),
        public_base,
    );
    if !kyc_storage.is_configured() {
        tracing::warn!(
            "KYC_HASH_SECRET is not set; KYC document uploads and signed URLs will be refused"
        );
    }

    infra::gc::spawn(db.clone());

    let app = api_routes::routes(mail_rate_limiter)
        .layer(Extension(db))
        .layer(Extension(kyc_storage))
        .layer(middleware::from_fn(move |req, next| {
            enforce_rate_limit(rate_limiter.clone(), req, next)
        }))
        .layer(middleware::from_fn(move |req, next| {
            enforce_concurrency(limiter.clone(), req, next)
        }))
        .layer(middleware::from_fn(enforce_csrf))
        .layer(cors)
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::HeaderName::from_static("content-security-policy"),
            HeaderValue::from_static("default-src 'none'"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            axum::http::header::HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
        ))
        .layer(TraceLayer::new_for_http());

    let addr = format!("0.0.0.0:{port}");
    println!("Server running on http://{addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
    .unwrap();
}
