
use axum::{
    Router,
    extract::DefaultBodyLimit,
    middleware,
    routing::{get, post},
};

use crate::api::{kyc, users};
use crate::infra::rate::{RateLimiter, enforce_rate_limit};

const AUTH_BODY_LIMIT: usize = 16 * 1024;
/// Two base64 images at up to 8MB decoded each (~11MB encoded), plus fields.
const KYC_BODY_LIMIT: usize = 24 * 1024 * 1024;

/// `mail_limiter` sits only on the endpoints that trigger outbound email —
/// see its construction in `engine.rs` for the rationale and numbers.
pub fn routes(mail_limiter: RateLimiter) -> Router {
    let register_limiter = mail_limiter.clone();
    let reset_limiter = mail_limiter;
    Router::new()
        .route(
            "/auth/register",
            post(users::register)
                .route_layer(middleware::from_fn(move |req, next| {
                    enforce_rate_limit(register_limiter.clone(), req, next)
                }))
                .layer(DefaultBodyLimit::max(AUTH_BODY_LIMIT)),
        )
        .route(
            "/auth/verify",
            post(users::verify).layer(DefaultBodyLimit::max(AUTH_BODY_LIMIT)),
        )
        .route(
            "/auth/login",
            post(users::login).layer(DefaultBodyLimit::max(AUTH_BODY_LIMIT)),
        )
        .route("/auth/session", get(users::session_handler))
        .route("/auth/logout", post(users::logout))
       
        .route(
            "/auth/password-reset/request",
            post(users::password_reset_request)
                .route_layer(middleware::from_fn(move |req, next| {
                    enforce_rate_limit(reset_limiter.clone(), req, next)
                }))
                .layer(DefaultBodyLimit::max(AUTH_BODY_LIMIT)),
        )
        .route(
            "/auth/password-reset/confirm",
            post(users::password_reset_confirm).layer(DefaultBodyLimit::max(AUTH_BODY_LIMIT)),
        )
        .route(
            "/kyc/submit",
            post(kyc::submit).layer(DefaultBodyLimit::max(KYC_BODY_LIMIT)),
        )
        .route("/kyc/status", get(kyc::status))
        // signed-URL document reads; the HMAC in the query string is the auth
        .route("/kyc/files/{*path}", get(kyc::file))
        .route("/kyc/admin/pending", get(kyc::admin_pending))
        .route("/kyc/admin/submissions/{id}", get(kyc::admin_detail))
        .route(
            "/kyc/admin/review",
            post(kyc::admin_review).layer(DefaultBodyLimit::max(AUTH_BODY_LIMIT)),
        )
        .layer(DefaultBodyLimit::max(30 * 1024 * 1024))
}
