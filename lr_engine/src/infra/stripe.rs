//! The PHP rail, both directions — Stripe.
//!
//! Deliberately the same shape as `infra::paypal`, function for function, so
//! that `api::lending` and `infra::payouts` can carry money over either rail
//! without knowing which one they are on. Where the two providers genuinely
//! differ, the difference is absorbed *here* rather than leaking upward.
//!
//! **In** — Checkout Sessions. The engine creates the session (so the amount
//! and the owning member are decided server-side, never by the page), the
//! member pays on checkout.stripe.com, and comes back with a session id.
//! `capture_session` then verifies with the secret key what was actually paid.
//! The client's claimed amount is never used; the centavos credited are
//! whatever Stripe says was collected.
//!
//! Note the one improvement over the PayPal flow this replaces: because the
//! engine creates the session, it can stamp `client_reference_id` with the
//! member's own id and refuse a session that belongs to somebody else. A bare
//! PayPal order id carries no such ownership claim.
//!
//! **Out** — Connect transfers. There is no Stripe equivalent of "pay this
//! consumer": money can only be sent to a *connected account*. So a member
//! onboards once through Express (`onboarding_link` → `account_status`),
//! which yields a Stripe-verified `acct_…`; nobody ever types a destination.
//! Transfers then go through `create_transfer`, keyed by the payout row's own
//! primary key.
//!
//! **The one real asymmetry, and how it is handled.** PayPal refuses a
//! `sender_batch_id` it has ever seen. Stripe only remembers an
//! `Idempotency-Key` for 24 hours, so a payout retried a day later would be
//! sent *twice* if the key were the only defence. It isn't: every transfer
//! also carries `transfer_group = payout_<id>`, and `create_transfer` looks
//! that group up before it posts anything. The lookup, not the header, is what
//! makes this safe indefinitely.
//!
//! Test and live are the same code on the same host — Stripe's key prefix
//! (`sk_test_` / `sk_live_`) is the whole switch, so there is no environment
//! variable to get wrong and no chance of pointing test credentials at a live
//! endpoint. Fails closed: unconfigured means every capture and every transfer
//! is refused.
//!
//! Env: STRIPE_SECRET_KEY, STRIPE_CONNECT_RETURN_URL and
//! STRIPE_CONNECT_REFRESH_URL (this engine's own callbacks),
//! STRIPE_WEBHOOK_SECRET (optional; only the webhook needs it),
//! STRIPE_CONNECT_COUNTRY (default "PH"), CLIENT_URL (where the member is sent
//! afterwards).

use std::sync::OnceLock;

use serde::Deserialize;

use super::rails::{CapturedPayment, PayoutOutcome, SubmitError};

/// Stripe has one host for test and live alike; the key decides which books
/// the call lands in. This is why there is no `STRIPE_ENV`.
const API_BASE: &str = "https://api.stripe.com";

/// Pinned so a Stripe-side default change can't silently reshape a response
/// this module parses. Bump deliberately, with the changelog open.
const API_VERSION: &str = "2024-06-20";

fn http() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .expect("reqwest client")
    })
}

fn secret_key() -> Option<String> {
    std::env::var("STRIPE_SECRET_KEY").ok().filter(|v| !v.is_empty())
}

/// Enough to talk to Stripe's API: captures and transfers need nothing else.
pub fn is_configured() -> bool {
    secret_key().is_some()
}

/// Enough to run Express onboarding, which additionally needs both registered
/// callbacks. Kept separate from `is_configured` for the same reason PayPal's
/// `can_connect` is: so the Settings card can say the button won't work
/// *before* it is clicked.
pub fn can_connect() -> bool {
    secret_key().is_some() && return_url().is_some() && refresh_url().is_some()
}

/// Where Stripe sends the member when onboarding finishes.
pub fn return_url() -> Option<String> {
    std::env::var("STRIPE_CONNECT_RETURN_URL").ok().filter(|v| !v.is_empty())
}

/// Where Stripe sends the member when the (single-use, ~5 minute) onboarding
/// link has expired and a fresh one is needed.
pub fn refresh_url() -> Option<String> {
    std::env::var("STRIPE_CONNECT_REFRESH_URL").ok().filter(|v| !v.is_empty())
}

/// The country a connected account is created under. Stripe decides what a
/// given country may do — transfers to some are restricted — so this is an
/// operator setting, not a constant.
fn connect_country() -> String {
    std::env::var("STRIPE_CONNECT_COUNTRY")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "PH".to_string())
}

/// True when the configured key is a live one. Only used for logging — no
/// behaviour branches on it, which is the point of Stripe's single host.
pub fn is_live() -> bool {
    secret_key().is_some_and(|k| k.starts_with("sk_live_"))
}

// --- request plumbing ------------------------------------------------------

/// Stripe's API is form-encoded, including its nested structures
/// (`metadata[user_id]`, `line_items[0][price_data][currency]`). Every call
/// therefore builds a flat slice of already-bracketed keys.
type Form<'a> = Vec<(&'a str, String)>;

/// Encodes a form body by hand rather than reaching for a helper.
///
/// The reason is `+`. A form parser reads a literal `+` as a space, and the
/// obvious encoders (including `Url`'s query serializer) leave it alone
/// because it is legal in a query string. That is fine until a member signs up
/// as `juan+test@gmail.com` — a real address, and exactly the shape people use
/// for testing — at which point the connected account is created against
/// `juan test@gmail.com` and Stripe's email to them goes nowhere.
///
/// So: percent-encode everything outside the unreserved set, `+` included.
fn form_body(form: &Form<'_>) -> String {
    fn encode(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for b in s.as_bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    out.push(*b as char)
                }
                _ => out.push_str(&format!("%{b:02X}")),
            }
        }
        out
    }

    form.iter()
        .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

/// The error envelope Stripe returns on every 4xx. Only the fields worth
/// acting on or logging are decoded.
#[derive(Deserialize)]
struct StripeErrorEnvelope {
    error: StripeError,
}

#[derive(Deserialize)]
struct StripeError {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

/// Pulls the machine-readable code and human message out of an error body.
/// Falls back to the raw text rather than swallowing it, because a Stripe
/// error we can't parse is exactly the one worth seeing in the logs.
fn decode_error(body: &str) -> (Option<String>, String) {
    match serde_json::from_str::<StripeErrorEnvelope>(body) {
        Ok(e) => {
            let message = e
                .error
                .message
                .unwrap_or_else(|| "Stripe refused the request".to_string());
            (e.error.code, message)
        }
        Err(_) => (None, body.chars().take(200).collect()),
    }
}

/// A POST to Stripe with the secret key, optionally carrying an idempotency
/// key. Returns the raw body on success so each caller decodes only the fields
/// it actually verifies.
async fn post(
    path: &str,
    form: &Form<'_>,
    idempotency_key: Option<&str>,
) -> Result<String, (reqwest::StatusCode, String)> {
    let key = secret_key().ok_or((
        reqwest::StatusCode::SERVICE_UNAVAILABLE,
        "Payments are not configured".to_string(),
    ))?;

    let mut req = http()
        .post(format!("{API_BASE}{path}"))
        .bearer_auth(key)
        .header("Stripe-Version", API_VERSION)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(form_body(form));
    if let Some(k) = idempotency_key {
        req = req.header("Idempotency-Key", k);
    }

    let res = req.send().await.map_err(|e| {
        tracing::error!("stripe POST {path}: {e}");
        (
            reqwest::StatusCode::SERVICE_UNAVAILABLE,
            "Stripe was unreachable".to_string(),
        )
    })?;

    let status = res.status();
    let body = res.text().await.map_err(|e| {
        tracing::error!("stripe POST {path} body: {e}");
        (
            reqwest::StatusCode::SERVICE_UNAVAILABLE,
            "Stripe was unreachable".to_string(),
        )
    })?;

    if status.is_success() {
        Ok(body)
    } else {
        Err((status, body))
    }
}

/// A GET against Stripe. `query` is appended verbatim when present.
async fn get(path: &str) -> Result<String, (reqwest::StatusCode, String)> {
    let key = secret_key().ok_or((
        reqwest::StatusCode::SERVICE_UNAVAILABLE,
        "Payments are not configured".to_string(),
    ))?;

    let res = http()
        .get(format!("{API_BASE}{path}"))
        .bearer_auth(key)
        .header("Stripe-Version", API_VERSION)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("stripe GET {path}: {e}");
            (
                reqwest::StatusCode::SERVICE_UNAVAILABLE,
                "Stripe was unreachable".to_string(),
            )
        })?;

    let status = res.status();
    let body = res.text().await.map_err(|e| {
        tracing::error!("stripe GET {path} body: {e}");
        (
            reqwest::StatusCode::SERVICE_UNAVAILABLE,
            "Stripe was unreachable".to_string(),
        )
    })?;

    if status.is_success() {
        Ok(body)
    } else {
        Err((status, body))
    }
}

/// Stripe ids are opaque but well-formed; anything else never reaches the
/// wire. Same guard `capture_order` applies to a PayPal order id, and for the
/// same reason: these values arrive from a browser.
fn valid_id(id: &str, prefix: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id.starts_with(prefix)
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

// ===========================================================================
// Money in: Checkout Sessions
// ===========================================================================

#[derive(Deserialize)]
struct SessionCreated {
    id: String,
    /// Where to send the member. Stripe omits this only for modes we don't use.
    #[serde(default)]
    url: Option<String>,
}

/// A created session: the member is sent to `url`, and comes back carrying
/// `id` for the engine to verify.
pub struct CheckoutSession {
    pub id: String,
    pub url: String,
}

/// Creates the session the member will pay on.
///
/// `user_id` is stamped into `client_reference_id` and into the payment's
/// metadata. That is the ownership claim `capture_session` checks — without
/// it, a member could confirm a deposit against somebody else's paid session
/// id and have it credited to their own lots.
///
/// `success_path`/`cancel_path` are app paths (e.g. "/lend"), not full URLs:
/// the origin comes from CLIENT_URL so a caller can't redirect a paying member
/// off-site.
pub async fn create_checkout_session(
    user_id: &str,
    centavos: i64,
    description: &str,
    success_path: &str,
    cancel_path: &str,
) -> Result<CheckoutSession, &'static str> {
    if centavos <= 0 || centavos > 1_000_000_000_000 {
        return Err("Invalid amount");
    }
    let origin = client_origin();
    // {CHECKOUT_SESSION_ID} is Stripe's own placeholder — it substitutes the
    // real id when it builds the redirect, so the engine never has to hand the
    // session id to the page in advance.
    //
    // The paths may already carry a query (a repayment names its loan), so the
    // separator is chosen rather than assumed — getting this wrong sends the
    // member back to a URL the app can't parse, after they have paid.
    let success_sep = if success_path.contains('?') { '&' } else { '?' };
    let cancel_sep = if cancel_path.contains('?') { '&' } else { '?' };
    let success_url = format!(
        "{origin}{success_path}{success_sep}stripe=success&session_id={{CHECKOUT_SESSION_ID}}"
    );
    let cancel_url = format!("{origin}{cancel_path}{cancel_sep}stripe=cancelled");

    let form: Form = vec![
        ("mode", "payment".to_string()),
        ("success_url", success_url),
        ("cancel_url", cancel_url),
        ("client_reference_id", user_id.to_string()),
        ("metadata[user_id]", user_id.to_string()),
        // Also on the PaymentIntent, so the ownership stamp survives on the
        // object the ledger's rail_ref actually names.
        ("payment_intent_data[metadata][user_id]", user_id.to_string()),
        ("line_items[0][quantity]", "1".to_string()),
        ("line_items[0][price_data][currency]", "php".to_string()),
        // Stripe takes the minor unit directly, so unlike the PayPal rail
        // there is no decimal string to parse and no chance of a float.
        ("line_items[0][price_data][unit_amount]", centavos.to_string()),
        (
            "line_items[0][price_data][product_data][name]",
            description.chars().take(120).collect(),
        ),
    ];

    let body = post("/v1/checkout/sessions", &form, None)
        .await
        .map_err(|(status, body)| {
            let (_, message) = decode_error(&body);
            tracing::error!("stripe session create {status}: {message}");
            "Stripe could not start the payment"
        })?;

    let session: SessionCreated = serde_json::from_str(&body).map_err(|e| {
        tracing::error!("stripe session decode: {e}");
        "Stripe sent a reply we couldn't read"
    })?;
    let url = session.url.ok_or("Stripe did not return a payment page")?;

    Ok(CheckoutSession { id: session.id, url })
}

#[derive(Deserialize)]
struct SessionRead {
    #[serde(default)]
    payment_status: String,
    #[serde(default)]
    currency: Option<String>,
    #[serde(default)]
    amount_total: Option<i64>,
    #[serde(default)]
    client_reference_id: Option<String>,
    /// The id of the PaymentIntent, unexpanded — this is the reference the
    /// ledger is keyed on.
    #[serde(default)]
    payment_intent: Option<String>,
}

/// Verifies a completed session and returns what was really paid.
///
/// Idempotency comes from the ledger's unique `rail_ref`: this function is
/// free to be called twice with the same session id, and the second credit
/// bounces off the schema — exactly as a re-sent PayPal capture does.
pub async fn capture_session(
    session_id: &str,
    expect_user: &str,
) -> Result<CapturedPayment, &'static str> {
    if !valid_id(session_id, "cs_") {
        return Err("Invalid payment reference");
    }

    let body = get(&format!("/v1/checkout/sessions/{session_id}"))
        .await
        .map_err(|(status, body)| {
            let (_, message) = decode_error(&body);
            tracing::error!("stripe session read {status}: {message}");
            "Payment could not be verified"
        })?;

    let session: SessionRead = serde_json::from_str(&body).map_err(|e| {
        tracing::error!("stripe session read decode: {e}");
        "Payment could not be verified"
    })?;

    // Ownership before amount: a paid session belonging to another member is
    // not a deposit this member gets to claim, however real the payment was.
    if session.client_reference_id.as_deref() != Some(expect_user) {
        tracing::warn!(session_id, "stripe session claimed by the wrong member");
        return Err("That payment doesn't belong to this account");
    }
    if session.payment_status != "paid" {
        return Err("Payment was not completed");
    }
    if session.currency.as_deref() != Some("php") {
        return Err("Payment must be in PHP");
    }
    let centavos = session.amount_total.unwrap_or(0);
    if centavos <= 0 {
        return Err("Invalid amount");
    }
    // The PaymentIntent, not the session, is the reference: a session is a
    // checkout attempt, the intent is the money.
    let intent = session
        .payment_intent
        .filter(|p| !p.is_empty())
        .ok_or("Payment could not be verified")?;

    Ok(CapturedPayment {
        capture_id: format!("stripe:{intent}"),
        centavos,
    })
}

/// The app origin, taken from CLIENT_URL's first entry for the same reason
/// `api::paypal::connect` does it: CLIENT_URL is a comma-separated list the
/// CORS layer allows, but a redirect can only go to one place.
fn client_origin() -> String {
    let raw = std::env::var("CLIENT_URL").unwrap_or_default();
    raw.split(',')
        .map(str::trim)
        .find(|s| !s.is_empty())
        .unwrap_or("http://localhost:5173")
        .trim_end_matches('/')
        .to_string()
}

// ===========================================================================
// Money out, part 1: connecting the member's own Stripe account
// ===========================================================================

/// What Stripe tells us about a member's connected account.
pub struct ConnectedStripe {
    /// Stripe's stable account id — the only thing a transfer is addressed to.
    pub account_id: String,
    /// Shown back to the member so they can see which account is linked.
    pub email: String,
    /// Stripe's own word on whether this account may receive money yet.
    /// Unlike PayPal's `verified`, this one is load-bearing: a transfer to an
    /// account without it is refused, so `destination` treats it as a gate.
    pub payouts_enabled: bool,
    /// Whether the member finished the onboarding form. False means the link
    /// was abandoned part-way and they need to be sent back through it.
    pub details_submitted: bool,
}

#[derive(Deserialize)]
struct AccountRead {
    id: String,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    payouts_enabled: bool,
    #[serde(default)]
    details_submitted: bool,
}

/// Creates the Express account a member will onboard into. Called once; the
/// resulting `acct_…` is stored and reused for every later transfer.
pub async fn create_express_account(email: &str) -> Result<String, &'static str> {
    let form: Form = vec![
        ("type", "express".to_string()),
        ("country", connect_country()),
        ("email", email.to_string()),
        // Transfers is the only capability the pool needs: members receive
        // money, they never charge anyone through us.
        ("capabilities[transfers][requested]", "true".to_string()),
    ];

    let body = post("/v1/accounts", &form, None)
        .await
        .map_err(|(status, body)| {
            let (_, message) = decode_error(&body);
            tracing::error!("stripe account create {status}: {message}");
            "Stripe could not create the account"
        })?;

    let account: AccountRead = serde_json::from_str(&body).map_err(|e| {
        tracing::error!("stripe account decode: {e}");
        "Stripe sent a reply we couldn't read"
    })?;
    Ok(account.id)
}

#[derive(Deserialize)]
struct AccountLink {
    url: String,
}

/// Appends `state` to a registered callback URL, preserving any query string
/// the operator already put on it.
fn with_state(url: &str, state: &str) -> String {
    let sep = if url.contains('?') { '&' } else { '?' };
    format!("{url}{sep}state={state}")
}

/// Where to send the member to complete (or resume) onboarding.
///
/// The link itself is single-use and expires in minutes, so it is minted fresh
/// every time rather than stored. `state` rides on the return and refresh URLs
/// because Stripe's redirect back carries nothing else that identifies the
/// member — see `api::stripe::connect` for why a cookie is not good enough
/// here.
pub async fn onboarding_link(account_id: &str, state: &str) -> Result<String, &'static str> {
    if !valid_id(account_id, "acct_") {
        return Err("Invalid account reference");
    }
    let return_url = return_url().ok_or("Payments are not configured")?;
    let refresh_url = refresh_url().ok_or("Payments are not configured")?;

    let form: Form = vec![
        ("account", account_id.to_string()),
        ("refresh_url", with_state(&refresh_url, state)),
        ("return_url", with_state(&return_url, state)),
        ("type", "account_onboarding".to_string()),
    ];

    let body = post("/v1/account_links", &form, None)
        .await
        .map_err(|(status, body)| {
            let (_, message) = decode_error(&body);
            tracing::error!("stripe account link {status}: {message}");
            "Stripe could not start the connection"
        })?;

    let link: AccountLink = serde_json::from_str(&body).map_err(|e| {
        tracing::error!("stripe account link decode: {e}");
        "Stripe sent a reply we couldn't read"
    })?;
    Ok(link.url)
}

/// The current state of a connected account. Called after onboarding returns,
/// and whenever the Settings card is read, because `payouts_enabled` can flip
/// on Stripe's side long after the member finished the form.
pub async fn account_status(account_id: &str) -> Result<ConnectedStripe, &'static str> {
    if !valid_id(account_id, "acct_") {
        return Err("Invalid account reference");
    }

    let body = get(&format!("/v1/accounts/{account_id}"))
        .await
        .map_err(|(status, body)| {
            let (_, message) = decode_error(&body);
            tracing::error!("stripe account read {status}: {message}");
            "Stripe did not share the account details"
        })?;

    let account: AccountRead = serde_json::from_str(&body).map_err(|e| {
        tracing::error!("stripe account read decode: {e}");
        "Stripe sent account details we couldn't read"
    })?;

    Ok(ConnectedStripe {
        account_id: account.id,
        email: account.email.unwrap_or_default(),
        payouts_enabled: account.payouts_enabled,
        details_submitted: account.details_submitted,
    })
}

// ===========================================================================
// Money out, part 2: the transfer itself
// ===========================================================================

#[derive(Deserialize)]
struct Transfer {
    id: String,
    #[serde(default)]
    amount_reversed: i64,
    #[serde(default)]
    amount: i64,
    #[serde(default)]
    reversed: bool,
    /// The charge created on the connected account — the member-visible half
    /// of the transfer, and the closest analogue to PayPal's transaction id.
    #[serde(default)]
    destination_payment: Option<String>,
}

#[derive(Deserialize)]
struct TransferList {
    #[serde(default)]
    data: Vec<Transfer>,
}

/// The `transfer_group` a payout's transfer is filed under. Derived from the
/// payout id, so it is stable across every retry of that one payout and
/// unique across all others.
fn transfer_group(payout_id: &str) -> String {
    format!("payout_{payout_id}")
}

/// Has this payout already been transferred? The defence that outlives
/// Stripe's 24-hour idempotency window.
async fn existing_transfer(payout_id: &str) -> Result<Option<Transfer>, SubmitError> {
    let group = transfer_group(payout_id);
    let body = get(&format!("/v1/transfers?transfer_group={group}&limit=1"))
        .await
        .map_err(|(status, body)| {
            let (_, message) = decode_error(&body);
            tracing::error!("stripe transfer lookup {status}: {message}");
            // Unknown is not "none": sending on a failed lookup is exactly the
            // double-pay this check exists to prevent.
            SubmitError::Retryable("Stripe was unreachable".to_string())
        })?;

    let list: TransferList = serde_json::from_str(&body).map_err(|e| {
        tracing::error!("stripe transfer lookup decode: {e}");
        SubmitError::Retryable("Stripe sent an unreadable reply".to_string())
    })?;
    Ok(list.data.into_iter().next())
}

/// Sends `centavos` to `account_id`, keyed by `payout_id`.
///
/// `payout_id` is the payout row's primary key, created in the database
/// *before* this call, and it does double duty: it is the `Idempotency-Key`
/// header (which covers retries within 24 hours) and the `transfer_group`
/// (which covers everything after). The group is checked *before* posting, so
/// however many times this is retried — after a timeout, a crash, a double
/// click, a week later — at most one transfer exists.
pub async fn create_transfer(
    payout_id: &str,
    account_id: &str,
    centavos: i64,
    note: &str,
) -> Result<String, SubmitError> {
    if centavos <= 0 {
        return Err(SubmitError::Refused("Invalid amount".to_string()));
    }
    if !valid_id(account_id, "acct_") {
        return Err(SubmitError::Refused("Invalid account reference".to_string()));
    }

    // One extra round trip per payout, deliberately on the happy path too:
    // being certain costs less than paying a member twice.
    if let Some(existing) = existing_transfer(payout_id).await? {
        tracing::warn!(payout_id, transfer = %existing.id, "stripe already has this transfer");
        return Err(SubmitError::AlreadySubmitted);
    }

    let form: Form = vec![
        ("amount", centavos.to_string()),
        ("currency", "php".to_string()),
        ("destination", account_id.to_string()),
        ("description", note.chars().take(200).collect()),
        ("transfer_group", transfer_group(payout_id)),
        ("metadata[payout_id]", payout_id.to_string()),
    ];

    match post("/v1/transfers", &form, Some(payout_id)).await {
        Ok(body) => {
            let transfer: Transfer = serde_json::from_str(&body).map_err(|e| {
                // The transfer may well exist; retryable is safe because the
                // group lookup above will find it next time round.
                tracing::error!("stripe transfer decode: {e}");
                SubmitError::Retryable("Stripe sent an unreadable reply".to_string())
            })?;
            tracing::info!(transfer = %transfer.id, "transfer submitted");
            Ok(transfer.id)
        }
        Err((status, body)) => {
            let (code, message) = decode_error(&body);
            tracing::error!("stripe transfer {status}: {message}");
            match code.as_deref() {
                // The platform's Stripe balance is short right now. That is a
                // liquidity condition, not a refusal — deposits will top it up
                // and the worker should keep trying.
                Some("balance_insufficient") => Err(SubmitError::Retryable(message)),
                // Stripe replayed a key with different parameters, which means
                // a transfer under this key already exists.
                Some("idempotency_key_in_use") => Err(SubmitError::AlreadySubmitted),
                _ if status.is_server_error()
                    || status == reqwest::StatusCode::TOO_MANY_REQUESTS =>
                {
                    Err(SubmitError::Retryable(message))
                }
                _ => Err(SubmitError::Refused(message)),
            }
        }
    }
}

/// Reads a transfer back.
///
/// A Stripe transfer has no "in flight" state — it either moved the money to
/// the connected account's balance or it did not — so the only outcomes this
/// can produce are `Paid` and `Returned`. There is no `Unclaimed`: Stripe has
/// nothing for a recipient to accept, which is one fewer state a member can be
/// stuck in than on the PayPal rail.
pub async fn transfer_status(transfer_id: &str) -> Result<PayoutOutcome, &'static str> {
    if !valid_id(transfer_id, "tr_") {
        return Err("Invalid transfer reference");
    }

    let body = get(&format!("/v1/transfers/{transfer_id}"))
        .await
        .map_err(|(status, body)| {
            let (_, message) = decode_error(&body);
            tracing::error!("stripe transfer status {status}: {message}");
            "Stripe was unreachable"
        })?;

    let transfer: Transfer = serde_json::from_str(&body).map_err(|e| {
        tracing::error!("stripe transfer status decode: {e}");
        "Stripe was unreachable"
    })?;

    // A reversal can be partial. Anything less than the whole amount leaves
    // the member genuinely paid, so only a full reversal undoes the payout.
    if transfer.reversed || (transfer.amount > 0 && transfer.amount_reversed >= transfer.amount) {
        return Ok(PayoutOutcome::Returned {
            item_id: Some(transfer.id),
            reason: "The transfer was reversed".to_string(),
        });
    }

    Ok(PayoutOutcome::Paid {
        item_id: transfer.id,
        transaction_id: transfer.destination_payment,
    })
}

// ===========================================================================
// Webhooks
// ===========================================================================

/// Verifies a `Stripe-Signature` header against the raw request body.
///
/// The signature covers `{timestamp}.{raw body}`, so the body must be the
/// exact bytes Stripe sent — re-serializing parsed JSON produces different
/// bytes and will never verify. `tolerance_secs` bounds how old a signature
/// may be, which is what stops a captured request being replayed forever.
pub fn verify_webhook(
    payload: &[u8],
    signature_header: &str,
    secret: &str,
    now: i64,
    tolerance_secs: i64,
) -> bool {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    use subtle::ConstantTimeEq;

    let mut timestamp: Option<i64> = None;
    let mut signatures: Vec<&str> = Vec::new();
    for part in signature_header.split(',') {
        match part.trim().split_once('=') {
            Some(("t", v)) => timestamp = v.parse().ok(),
            // v1 only. Stripe's older v0 scheme is not accepted, so a
            // downgrade can't be used to get a weaker signature past this.
            Some(("v1", v)) => signatures.push(v),
            _ => {}
        }
    }

    let Some(timestamp) = timestamp else {
        return false;
    };
    if (now - timestamp).abs() > tolerance_secs {
        return false;
    }
    if signatures.is_empty() {
        return false;
    }

    let mut mac = match Hmac::<Sha256>::new_from_slice(secret.as_bytes()) {
        Ok(mac) => mac,
        Err(_) => return false,
    };
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(payload);
    let expected = hex::encode(mac.finalize().into_bytes());

    // Constant time, and every candidate is checked: Stripe sends more than
    // one v1 during a secret rotation.
    signatures
        .iter()
        .any(|s| bool::from(s.as_bytes().ct_eq(expected.as_bytes())))
}

/// The webhook secret, if one is configured. Absent means the webhook route
/// refuses everything rather than trusting unsigned posts.
pub fn webhook_secret() -> Option<String> {
    std::env::var("STRIPE_WEBHOOK_SECRET").ok().filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{Form, form_body, transfer_group, valid_id, verify_webhook};

    #[test]
    fn form_encoding_survives_the_characters_that_matter() {
        // The one that bites: a `+` left alone reaches Stripe as a space, and
        // the member's connected account is created against the wrong address.
        let form: Form = vec![("email", "juan+test@gmail.com".to_string())];
        assert_eq!(form_body(&form), "email=juan%2Btest%40gmail.com");

        // Stripe's nested keys survive as brackets it accepts percent-encoded.
        let form: Form = vec![("metadata[user_id]", "abc-123".to_string())];
        assert_eq!(form_body(&form), "metadata%5Buser_id%5D=abc-123");

        // Spaces are %20, never `+`, so they can't be confused with the above.
        let form: Form = vec![
            ("description", "PrimeLendRow pool deposit".to_string()),
            ("amount", "150000".to_string()),
        ];
        assert_eq!(
            form_body(&form),
            "description=PrimeLendRow%20pool%20deposit&amount=150000"
        );

        // An ampersand in a value can't inject a new field.
        let form: Form = vec![("description", "a&amount=1".to_string())];
        assert_eq!(form_body(&form), "description=a%26amount%3D1");

        // Non-ASCII is encoded per byte, not dropped.
        let form: Form = vec![("description", "₱500".to_string())];
        assert_eq!(form_body(&form), "description=%E2%82%B1500");
    }

    fn sign(payload: &[u8], secret: &str, timestamp: i64) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(timestamp.to_string().as_bytes());
        mac.update(b".");
        mac.update(payload);
        format!("t={timestamp},v1={}", hex::encode(mac.finalize().into_bytes()))
    }

    #[test]
    fn accepts_a_real_signature_and_nothing_else() {
        let payload = br#"{"id":"evt_1","type":"account.updated"}"#;
        let secret = "whsec_test_secret";
        let now = 1_750_000_000;

        assert!(verify_webhook(payload, &sign(payload, secret, now), secret, now, 300));

        // wrong secret
        assert!(!verify_webhook(payload, &sign(payload, "whsec_other", now), secret, now, 300));
        // body changed after signing — the whole point of hashing raw bytes
        let tampered = br#"{"id":"evt_1","type":"account.deleted"}"#;
        assert!(!verify_webhook(tampered, &sign(payload, secret, now), secret, now, 300));
        // replayed an hour later
        assert!(!verify_webhook(payload, &sign(payload, secret, now - 3600), secret, now, 300));
        // no signature at all
        assert!(!verify_webhook(payload, "t=1750000000", secret, now, 300));
        assert!(!verify_webhook(payload, "", secret, now, 300));
    }

    #[test]
    fn accepts_any_of_several_signatures_during_rotation() {
        let payload = br#"{"id":"evt_2"}"#;
        let secret = "whsec_current";
        let now = 1_750_000_000;
        let good = sign(payload, secret, now);
        let v1 = good.split_once("v1=").unwrap().1;
        let header = format!("t={now},v1=deadbeef,v1={v1}");
        assert!(verify_webhook(payload, &header, secret, now, 300));
    }

    #[test]
    fn rejects_ids_that_are_not_stripe_ids() {
        assert!(valid_id("acct_1A2b3C", "acct_"));
        assert!(valid_id("cs_test_a1b2", "cs_"));
        assert!(!valid_id("acct_1A2b3C", "tr_"));
        // path traversal and separators never reach the wire
        assert!(!valid_id("acct_../../v1/accounts", "acct_"));
        assert!(!valid_id("acct_1?expand[]=x", "acct_"));
        assert!(!valid_id("", "acct_"));
    }

    #[test]
    fn groups_transfers_by_their_payout() {
        assert_eq!(
            transfer_group("0f1c9a2e-1111-2222-3333-444455556666"),
            "payout_0f1c9a2e-1111-2222-3333-444455556666"
        );
    }
}
