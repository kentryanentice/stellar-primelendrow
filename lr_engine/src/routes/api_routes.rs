
use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
};

use crate::api::{users};

const AUTH_BODY_LIMIT: usize = 16 * 1024;

pub fn routes() -> Router {
    Router::new()
        .route(
            "/auth/register",
            post(users::register).layer(DefaultBodyLimit::max(AUTH_BODY_LIMIT)),
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
            post(users::password_reset_request).layer(DefaultBodyLimit::max(AUTH_BODY_LIMIT)),
        )
        .route(
            "/auth/password-reset/confirm",
            post(users::password_reset_confirm).layer(DefaultBodyLimit::max(AUTH_BODY_LIMIT)),
        )
        .layer(DefaultBodyLimit::max(30 * 1024 * 1024))
}
