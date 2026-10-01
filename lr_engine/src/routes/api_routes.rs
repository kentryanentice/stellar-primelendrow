
use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::{StatusCode, header},
    middleware,
    response::Response,
    routing::{get, post},
};

use crate::api::{credit, kyc, lending, paypal, stripe, users, wallets};
use crate::infra::rate::{RateLimiter, enforce_rate_limit};

const AUTH_BODY_LIMIT: usize = 16 * 1024;
/// Stripe event bodies are small JSON, but a few (a fully expanded account)
/// run larger than a lending request. Generous enough not to reject a real
/// event, tight enough that an unsigned post can't be used to buffer size.
const WEBHOOK_BODY_LIMIT: usize = 256 * 1024;
/// Lending mutations are small JSON bodies (an order id, an amount, up to
/// three guarantor asks) — same ceiling as the auth endpoints.
const LENDING_BODY_LIMIT: usize = 16 * 1024;
/// Two base64 images at up to 8MB decoded each (~11MB encoded), plus fields.
const KYC_BODY_LIMIT: usize = 24 * 1024 * 1024;

/// `mail_limiter` sits only on the endpoints that trigger outbound email —
/// see its construction in `engine.rs` for the rationale and numbers.
pub fn routes(mail_limiter: RateLimiter) -> Router {
    let register_limiter = mail_limiter.clone();
    let reset_limiter = mail_limiter;
    let api = Router::new()
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
        // Sign in with Google: browser redirects, not API calls (see
        // `users::google`), so they sit on the payload exemption list.
        .route("/auth/google/start", get(users::google_start))
        .route("/auth/google/callback", get(users::google_callback))
       
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
        // Public identifiers the web app loads at runtime (see `api::config`).
        .route("/config", get(crate::api::config::public_config))
        .route("/credit/score", get(credit::status))
        .route("/credit/history", get(lending::credit_history))
        .route("/pool", get(lending::pool_summary))
        // The public loan book: no session, GET only, anonymised by
        // construction (see `lending::public`). The global rate limiter is
        // what stands in front of it.
        .route("/public/loans", get(lending::public_loans))
        .route("/public/loans/{loan_id}", get(lending::public_loan))
        .route(
            "/pool/deposit",
            post(lending::deposit).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        .route(
            "/pool/withdraw",
            post(lending::withdraw).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        .route(
            "/pool/deposits",
            post(lending::deposits_list).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        .route(
            "/pool/transactions",
            post(lending::transactions_list).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        .route("/loans", get(lending::loans_list))
        .route(
            "/loans/history",
            post(lending::loans_history).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        .route(
            "/loans/payments",
            post(lending::payments_list).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        .route("/loans/quote", get(lending::loan_quote))
        .route(
            "/loans/apply",
            post(lending::apply).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        // The borrower withdrawing their own application (035). Refused once a
        // guarantor has accepted, or once the coins are in the vault — see
        // `lending::cancel` for why those two are different kinds of refusal.
        .route(
            "/loans/cancel",
            post(lending::loan_cancel).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        .route(
            "/loans/repay",
            post(lending::repay).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        .route(
            "/collateral/confirm",
            post(lending::collateral_confirm).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        .route("/loans/{loan_id}/collateral", get(lending::collateral_record))
        .route(
            "/loans/payout",
            post(lending::payout_request).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        .route("/payouts", get(lending::payouts_list))
        // "Log in with PayPal": the member authorises on PayPal's own domain
        // and is redirected back to /paypal/callback, which is a GET (so the
        // CSRF guard passes it) and identifies them from the single-use
        // `state` row rather than from a cookie the redirect may not carry.
        // The order is created here, not in the browser, so it carries the
        // engine's amount and the caller's ownership stamp (`custom_id`).
        .route(
            "/paypal/order",
            post(paypal::create_order).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        // The member closed PayPal's window: release the hold that started
        // payment has on their deposit limit. Charges nothing either way.
        .route(
            "/paypal/order/cancel",
            post(paypal::cancel_order).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        .route("/paypal/connect", get(paypal::start))
        .route("/paypal/callback", get(paypal::callback))
        .route("/paypal/account", get(paypal::status))
        .route(
            "/paypal/disconnect",
            post(paypal::disconnect).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        // Stripe Connect onboarding: the member fills in Stripe's own form and
        // is redirected back to /stripe/return, which — like the PayPal
        // callback — is a GET (so the CSRF guard passes it) and identifies
        // them from the single-use `state` row rather than from a cookie the
        // redirect may not carry.
        .route("/stripe/connect", get(stripe::start))
        .route("/stripe/return", get(stripe::callback))
        .route("/stripe/refresh", get(stripe::refresh))
        .route("/stripe/account", get(stripe::status))
        .route(
            "/stripe/disconnect",
            post(stripe::disconnect).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        // Creates the hosted page a deposit or repayment is paid on. The
        // engine owns the amount and stamps the paying member into the
        // session, which is what /pool/deposit later checks it against.
        .route(
            "/stripe/checkout",
            post(stripe::checkout).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        // Came back from Stripe without paying: close the page and release the
        // hold it had on the member's deposit limit. Charges nothing.
        .route(
            "/stripe/checkout/cancel",
            post(stripe::checkout_cancel).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        // Signed by Stripe, not by a session — the CSRF guard only enforces on
        // requests carrying a session cookie, and this one never does. The
        // handler refuses anything whose signature doesn't verify against
        // STRIPE_WEBHOOK_SECRET.
        .route(
            "/stripe/webhook",
            post(stripe::webhook).layer(DefaultBodyLimit::max(WEBHOOK_BODY_LIMIT)),
        )
        .route("/guarantors/invites", get(lending::guarantor_invites))
        .route(
            "/guarantors/respond",
            post(lending::guarantor_respond).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        .route(
            "/lending/admin/fx-rate",
            post(lending::set_fx_rate).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        // The operator's lending console: every loan, declaring a default, and
        // draining the vault outbox. All four are `require_admin` inside the
        // handler — the route table is a map, never the authorization.
        .route(
            "/lending/admin/loans",
            post(lending::admin_loans).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        // Where every repayment's interest went, and to whom.
        .route(
            "/lending/admin/interest",
            post(lending::admin_interest).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        .route(
            "/lending/admin/loans/default",
            post(lending::loan_default).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        // The way back from a default (033), deliberately two calls rather than
        // one: `reopen` lets the borrower pay their arrears through the normal
        // rail, and `reconcile` accepts the result once they actually have.
        // Neither one moves money on an admin's say-so.
        .route(
            "/lending/admin/loans/reopen",
            post(lending::loan_reopen).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        .route(
            "/lending/admin/loans/reconcile",
            post(lending::loan_mark_paid).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        .route(
            "/lending/admin/actions",
            post(lending::actions_list).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        .route(
            "/lending/admin/actions/prepare",
            post(lending::action_prepare).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        .route(
            "/lending/admin/actions/confirm",
            post(lending::action_confirm).layer(DefaultBodyLimit::max(LENDING_BODY_LIMIT)),
        )
        .route("/wallets", get(wallets::list))
        .route(
            "/wallets/challenge",
            post(wallets::challenge).layer(DefaultBodyLimit::max(AUTH_BODY_LIMIT)),
        )
        .route(
            "/wallets/connect",
            post(wallets::connect).layer(DefaultBodyLimit::max(AUTH_BODY_LIMIT)),
        )
        .route(
            "/wallets/disconnect",
            post(wallets::disconnect).layer(DefaultBodyLimit::max(AUTH_BODY_LIMIT)),
        )
        .route(
            "/kyc/admin/pending",
            post(kyc::admin_pending).layer(DefaultBodyLimit::max(AUTH_BODY_LIMIT)),
        )
        .route("/kyc/admin/submissions/{id}", get(kyc::admin_detail))
        .route(
            "/kyc/admin/review",
            post(kyc::admin_review).layer(DefaultBodyLimit::max(AUTH_BODY_LIMIT)),
        )
        // A real path called with the wrong method answers exactly like a path
        // that doesn't exist — an empty 404 — where axum's default 405, and
        // its `Allow` header, would confirm the route is there and list the
        // methods it takes. Registered last: it only reaches routes added above.
        .method_not_allowed_fallback(not_found)
        .layer(DefaultBodyLimit::max(30 * 1024 * 1024));

    // ...and without the `Allow` header axum still attaches to that answer,
    // which would list the route's methods all the same. It is added outside
    // anything `Router::layer` wraps (that only reaches the handlers), so the
    // header is stripped from around the whole router instead. Nothing this
    // API sends on purpose carries one.
    Router::new()
        .fallback_service(api)
        .layer(middleware::map_response(strip_allow))
}

/// The same bare 404 axum answers an unknown path with.
async fn not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}

async fn strip_allow(mut res: Response) -> Response {
    res.headers_mut().remove(header::ALLOW);
    res
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    use super::*;

    /// Status, whether an `Allow` header came back, and the body.
    async fn call(method: &str, path: &str) -> (StatusCode, bool, Vec<u8>) {
        let limiter = RateLimiter::new(100, 100, Duration::from_secs(60), "test".into());
        let res = routes(limiter)
            .oneshot(Request::builder().method(method).uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = res.status();
        let allow = res.headers().contains_key(axum::http::header::ALLOW);
        let body = axum::body::to_bytes(res.into_body(), 1024).await.unwrap().to_vec();
        (status, allow, body)
    }

    #[tokio::test]
    async fn a_wrong_method_is_indistinguishable_from_a_missing_route() {
        let missing = call("GET", "/no-such-route").await;
        assert_eq!(missing, (StatusCode::NOT_FOUND, false, Vec::new()));
        // Real routes, wrong methods: a POST-only route read, a GET-only
        // route written to.
        assert_eq!(call("GET", "/pool/deposit").await, missing);
        assert_eq!(call("DELETE", "/auth/google/start").await, missing);
    }
}
