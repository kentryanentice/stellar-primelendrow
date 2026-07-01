pub mod api;
pub mod infra;
mod routes;

use std::{env, sync::Arc, time::Duration};

use axum::http::HeaderValue;
use axum::{
    extract::Extension,
    http::Method,
    middleware::{self},
};
use dashmap::DashMap;
use dotenvy::dotenv;
use infra::csrf::enforce_csrf;
use infra::db::init_db_pool;
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
    let rate_limiter = RateLimiter::new(1000, Duration::from_secs(60), device_secret);

    if env::var("OTP_HASH_SECRET").map(|v| v.is_empty()).unwrap_or(true) {
        tracing::warn!(
            "OTP_HASH_SECRET is not set; verification codes are hashed with an empty key"
        );
    }

    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());

    let client_url = env::var("CLIENT_URL").unwrap_or_else(|_| "http://localhost:5173".to_string());
    let origin = HeaderValue::from_str(&client_url).expect("Invalid CLIENT_URL");

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list([origin]))
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

    let nonce_store: api::verified::NonceStore = Arc::new(DashMap::new());

    let db_pool = init_db_pool().await;

    infra::gc::spawn(db_pool.clone());

    let app = api_routes::routes()
        .layer(Extension(nonce_store))
        .layer(Extension(db_pool))
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
