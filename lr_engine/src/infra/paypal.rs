//! The PHP rail, both directions.
//!
//! **In** — server-side order capture. The frontend renders PayPal Buttons
//! with the *public* client id and hands the engine nothing but an order id.
//! Everything that decides money — capturing the order, checking
//! status/currency/amount — happens here with the client SECRET, which never
//! leaves the backend. The client's claimed amount is never used: the centavos
//! credited are whatever PayPal says was actually captured.
//!
//! **Out** — Payouts. A member connects their own PayPal through "Log in with
//! PayPal" (`connect_url` → `exchange_code`), which yields a PayPal-verified
//! payer id; nobody ever types a destination. Transfers then go through
//! `create_payout`, whose `sender_batch_id` is the payout row's own primary
//! key: PayPal refuses a batch id it has already seen, so a retried
//! submission bounces off PayPal rather than paying twice.
//!
//! Sandbox and live are the same code on different hosts — `api_base` and
//! `web_base` switch on PAYPAL_ENV and nothing else does. The request shapes,
//! the statuses and the idempotency rules are identical, which is the point:
//! what is exercised in sandbox is what runs in production.
//!
//! Env: PAYPAL_CLIENT_ID, PAYPAL_SECRET, PAYPAL_ENV ("live" | anything else
//! = sandbox), PAYPAL_RETURN_URL (this engine's own callback, registered with
//! the PayPal app), CLIENT_URL (where the member is sent afterwards).
//! Fails closed: unconfigured means every capture and every payout is refused.

use std::sync::OnceLock;

use serde::Deserialize;

pub struct CapturedPayment {
    /// PayPal's capture id — the ledger's `rail_ref`, unique by schema.
    pub capture_id: String,
    /// Whole centavos actually captured, parsed without ever touching floats.
    pub centavos: i64,
}

fn http() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .expect("reqwest client")
    })
}

fn api_base() -> &'static str {
    static BASE: OnceLock<&'static str> = OnceLock::new();
    BASE.get_or_init(|| {
        match std::env::var("PAYPAL_ENV").as_deref() {
            Ok("live") => "https://api-m.paypal.com",
            _ => "https://api-m.sandbox.paypal.com",
        }
    })
}

/// PayPal's *user-facing* host — where the member is sent to authorise the
/// connection. Distinct from `api_base`, which is the machine API.
fn web_base() -> &'static str {
    static BASE: OnceLock<&'static str> = OnceLock::new();
    BASE.get_or_init(|| {
        match std::env::var("PAYPAL_ENV").as_deref() {
            Ok("live") => "https://www.paypal.com",
            _ => "https://www.sandbox.paypal.com",
        }
    })
}

/// This engine's own callback, as registered in the PayPal app's "Log in with
/// PayPal" return URLs. PayPal refuses any redirect_uri it doesn't know, so a
/// mismatch here fails the connect flow rather than sending members anywhere
/// unexpected.
pub fn return_url() -> Option<String> {
    std::env::var("PAYPAL_RETURN_URL").ok().filter(|v| !v.is_empty())
}

fn credentials() -> Option<(String, String)> {
    let id = std::env::var("PAYPAL_CLIENT_ID").ok()?;
    let secret = std::env::var("PAYPAL_SECRET").ok()?;
    if id.is_empty() || secret.is_empty() {
        return None;
    }
    Some((id, secret))
}

/// Enough to talk to PayPal's API: captures and payouts need nothing else.
pub fn is_configured() -> bool {
    credentials().is_some()
}

/// Enough to run "Log in with PayPal", which additionally needs the registered
/// return URL. Kept separate from `is_configured` so the Settings card can say
/// the connect button won't work *before* it is clicked — credentials alone
/// would report ready and then fail in `connect_url`.
pub fn can_connect() -> bool {
    credentials().is_some() && return_url().is_some()
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

async fn access_token() -> Result<String, &'static str> {
    let (id, secret) = credentials().ok_or("Payments are not configured")?;
    let res = http()
        .post(format!("{}/v1/oauth2/token", api_base()))
        .basic_auth(id, Some(secret))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body("grant_type=client_credentials")
        .send()
        .await
        .map_err(|e| {
            tracing::error!("paypal oauth: {e}");
            "Payment provider unreachable"
        })?;
    if !res.status().is_success() {
        tracing::error!("paypal oauth status {}", res.status());
        return Err("Payment provider rejected credentials");
    }
    res.json::<TokenResponse>()
        .await
        .map(|t| t.access_token)
        .map_err(|e| {
            tracing::error!("paypal oauth body: {e}");
            "Payment provider unreachable"
        })
}

// --- order/capture response shapes (only the fields we verify) ------------

#[derive(Deserialize)]
struct OrderResponse {
    status: String,
    #[serde(default)]
    purchase_units: Vec<PurchaseUnit>,
}

#[derive(Deserialize)]
struct PurchaseUnit {
    payments: Option<Payments>,
}

#[derive(Deserialize)]
struct Payments {
    #[serde(default)]
    captures: Vec<Capture>,
}

#[derive(Deserialize)]
struct Capture {
    id: String,
    status: String,
    amount: Money,
}

#[derive(Deserialize)]
struct Money {
    currency_code: String,
    value: String,
}

/// "1500.00" -> 150000 centavos. String math only — a peso amount must never
/// pass through an f64 (Lesson: no floats near money). Rejects more than two
/// decimals rather than rounding: PayPal never sends sub-centavo PHP.
pub fn parse_centavos(value: &str) -> Result<i64, &'static str> {
    let (whole, frac) = match value.split_once('.') {
        Some((w, f)) => (w, f),
        None => (value, ""),
    };
    if whole.is_empty() || whole.len() > 12 || !whole.chars().all(|c| c.is_ascii_digit()) {
        return Err("Invalid amount");
    }
    if frac.len() > 2 || !frac.chars().all(|c| c.is_ascii_digit()) {
        return Err("Invalid amount");
    }
    let whole: i64 = whole.parse().map_err(|_| "Invalid amount")?;
    let frac_val: i64 = if frac.is_empty() {
        0
    } else if frac.len() == 1 {
        frac.parse::<i64>().map_err(|_| "Invalid amount")? * 10
    } else {
        frac.parse().map_err(|_| "Invalid amount")?
    };
    Ok(whole * 100 + frac_val)
}

fn completed_php_capture(order: OrderResponse) -> Result<CapturedPayment, &'static str> {
    let capture = order
        .purchase_units
        .into_iter()
        .filter_map(|u| u.payments)
        .flat_map(|p| p.captures)
        .find(|c| c.status == "COMPLETED")
        .ok_or("Payment was not completed")?;
    if capture.amount.currency_code != "PHP" {
        return Err("Payment must be in PHP");
    }
    let centavos = parse_centavos(&capture.amount.value)?;
    if centavos <= 0 {
        return Err("Invalid amount");
    }
    Ok(CapturedPayment {
        capture_id: capture.id,
        centavos,
    })
}

/// Captures an approved order and returns the verified capture.
///
/// Idempotency, two layers: PayPal answers `ORDER_ALREADY_CAPTURED` (422) for
/// a re-capture, in which case the order is re-fetched and its existing
/// completed capture returned; and the ledger's unique `rail_ref` refuses a
/// double credit even if both layers were somehow fooled.
pub async fn capture_order(order_id: &str) -> Result<CapturedPayment, &'static str> {
    if order_id.is_empty()
        || order_id.len() > 64
        || !order_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err("Invalid order reference");
    }
    let token = access_token().await?;

    let res = http()
        .post(format!("{}/v2/checkout/orders/{order_id}/capture", api_base()))
        .bearer_auth(&token)
        .header("Content-Type", "application/json")
        // return=representation: the capture response carries the full
        // purchase_units so no second round-trip is needed on success.
        .header("Prefer", "return=representation")
        .body("{}")
        .send()
        .await
        .map_err(|e| {
            tracing::error!("paypal capture: {e}");
            "Payment provider unreachable"
        })?;

    let status = res.status();
    if status.is_success() {
        let order = res.json::<OrderResponse>().await.map_err(|e| {
            tracing::error!("paypal capture body: {e}");
            "Payment provider unreachable"
        })?;
        return completed_php_capture(order);
    }

    // Already captured (a retry, a double click, a resent request): fetch the
    // order and return the capture that already happened.
    if status == reqwest::StatusCode::UNPROCESSABLE_ENTITY {
        let res = http()
            .get(format!("{}/v2/checkout/orders/{order_id}", api_base()))
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| {
                tracing::error!("paypal order fetch: {e}");
                "Payment provider unreachable"
            })?;
        if res.status().is_success() {
            let order = res.json::<OrderResponse>().await.map_err(|e| {
                tracing::error!("paypal order body: {e}");
                "Payment provider unreachable"
            })?;
            if order.status == "COMPLETED" {
                return completed_php_capture(order);
            }
        }
        return Err("Payment was not completed");
    }

    tracing::error!("paypal capture status {status}");
    Err("Payment could not be verified")
}

// ===========================================================================
// Money out, part 1: connecting the member's own PayPal ("Log in with PayPal")
// ===========================================================================

/// What PayPal tells us about the account a member just authorised.
pub struct ConnectedPaypal {
    /// PayPal's stable account id — the only thing a payout is addressed to.
    pub payer_id: String,
    /// Shown back to the member so they can see which account is linked.
    pub email: String,
    /// PayPal's own word on whether the account is verified.
    pub verified: bool,
}

/// Percent-encodes a query string the same way a URL would, without pulling in
/// an encoder: `Url` already has to do this correctly, so borrow its answer.
fn query_string(params: &[(&str, &str)]) -> String {
    reqwest::Url::parse_with_params("https://x.invalid/", params)
        .ok()
        .and_then(|u| u.query().map(str::to_owned))
        .unwrap_or_default()
}

/// Where to send the member to authorise the connection. `state` is the
/// single-use token that ties the callback back to them.
///
/// The scopes are the minimum that yields a payout destination: `openid` and
/// `email` identify the account, and PayPal's `paypalattributes` scope is what
/// adds `payer_id` to the identity response. Without it the callback gets a
/// user with no address to pay.
pub fn connect_url(state: &str) -> Option<String> {
    let (client_id, _) = credentials()?;
    let redirect_uri = return_url()?;
    let query = query_string(&[
        ("flowEntry", "static"),
        ("client_id", &client_id),
        ("response_type", "code"),
        (
            "scope",
            "openid email https://uri.paypal.com/services/paypalattributes",
        ),
        ("redirect_uri", &redirect_uri),
        ("state", state),
    ]);
    Some(format!("{}/connect?{query}", web_base()))
}

#[derive(Deserialize)]
struct UserTokenResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct UserInfo {
    #[serde(default)]
    payer_id: Option<String>,
    #[serde(default)]
    emails: Vec<UserEmail>,
    #[serde(default)]
    verified_account: Option<bool>,
}

#[derive(Deserialize)]
struct UserEmail {
    value: String,
    #[serde(default)]
    primary: bool,
}

/// Exchanges the authorization code for the member's identity. Runs entirely
/// server-side with the client secret; the browser only ever carries the
/// opaque code.
pub async fn exchange_code(code: &str) -> Result<ConnectedPaypal, &'static str> {
    if code.is_empty() || code.len() > 512 {
        return Err("Invalid authorization code");
    }
    let (id, secret) = credentials().ok_or("Payments are not configured")?;
    let redirect_uri = return_url().ok_or("Payments are not configured")?;

    let body = query_string(&[
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", &redirect_uri),
    ]);
    let res = http()
        .post(format!("{}/v1/oauth2/token", api_base()))
        .basic_auth(&id, Some(&secret))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("paypal connect token: {e}");
            "PayPal was unreachable"
        })?;
    if !res.status().is_success() {
        tracing::error!("paypal connect token status {}", res.status());
        return Err("PayPal did not accept the connection");
    }
    let token = res
        .json::<UserTokenResponse>()
        .await
        .map_err(|e| {
            tracing::error!("paypal connect token body: {e}");
            "PayPal was unreachable"
        })?
        .access_token;

    // The member's own token, not the platform's — this reads their identity,
    // and nothing else.
    let res = http()
        .get(format!(
            "{}/v1/identity/oauth2/userinfo?schema=paypalv1.1",
            api_base()
        ))
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("paypal userinfo: {e}");
            "PayPal was unreachable"
        })?;
    if !res.status().is_success() {
        tracing::error!("paypal userinfo status {}", res.status());
        return Err("PayPal did not share the account details");
    }
    let info: UserInfo = res.json().await.map_err(|e| {
        tracing::error!("paypal userinfo body: {e}");
        "PayPal was unreachable"
    })?;

    // Fail closed: an identity with no payer id is an account we cannot pay,
    // and storing it would leave a member believing they were connected.
    let payer_id = info
        .payer_id
        .filter(|p| !p.is_empty() && p.len() <= 32)
        .ok_or("PayPal didn't return an account id — check the app's paypalattributes scope")?;
    let email = info
        .emails
        .iter()
        .find(|e| e.primary)
        .or_else(|| info.emails.first())
        .map(|e| e.value.clone())
        .unwrap_or_default();

    Ok(ConnectedPaypal {
        payer_id,
        email,
        verified: info.verified_account.unwrap_or(false),
    })
}

// ===========================================================================
// Money out, part 2: the payout itself
// ===========================================================================

/// 150000 -> "1500.00". The inverse of `parse_centavos`, string math only:
/// the value PayPal is asked to send must be derived from the integer the
/// books hold, never from a float that happens to print the same.
pub fn format_centavos(centavos: i64) -> String {
    format!("{}.{:02}", centavos / 100, (centavos % 100).abs())
}

/// Where a payout has got to. Only `Paid` is allowed to move the books.
pub enum PayoutOutcome {
    /// PayPal has the money and the recipient has it.
    Paid { item_id: String, transaction_id: Option<String> },
    /// Accepted, still moving.
    Pending { item_id: Option<String> },
    /// Sent, but the recipient hasn't accepted it. PayPal returns these
    /// automatically after 30 days.
    Unclaimed { item_id: Option<String> },
    /// Came back — refused, returned or reversed. The money is ours again.
    Returned { item_id: Option<String>, reason: String },
    /// PayPal refused it outright.
    Failed { reason: String },
}

#[derive(Deserialize)]
struct PayoutBatchResponse {
    batch_header: BatchHeader,
    #[serde(default)]
    items: Vec<PayoutItem>,
}

#[derive(Deserialize)]
struct BatchHeader {
    payout_batch_id: String,
    batch_status: String,
}

#[derive(Deserialize)]
struct PayoutItem {
    #[serde(default)]
    payout_item_id: Option<String>,
    #[serde(default)]
    transaction_id: Option<String>,
    #[serde(default)]
    transaction_status: Option<String>,
    #[serde(default)]
    errors: Option<PayoutItemError>,
}

#[derive(Deserialize)]
struct PayoutItemError {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

/// Submitting a payout can fail in two very different ways, and the caller
/// must not treat them alike.
pub enum SubmitError {
    /// Nothing was sent — safe to retry with the same batch id.
    Retryable(String),
    /// PayPal refused this payout for good; retrying changes nothing.
    Refused(String),
    /// PayPal has already seen this batch id, so the money may well be on its
    /// way. NEVER retry with a new id: look the batch up by its sender batch
    /// id in the PayPal dashboard and reconcile.
    AlreadySubmitted,
}

/// Sends `centavos` to `payer_id`, keyed by `sender_batch_id`.
///
/// `sender_batch_id` is the payout row's primary key, created in the database
/// *before* this call. That ordering is the whole safety argument: PayPal
/// rejects a batch id it has seen before, so however many times this is
/// retried — after a timeout, a crash, a double click — at most one transfer
/// exists.
pub async fn create_payout(
    sender_batch_id: &str,
    payer_id: &str,
    centavos: i64,
    note: &str,
) -> Result<String, SubmitError> {
    if centavos <= 0 {
        return Err(SubmitError::Refused("Invalid amount".to_string()));
    }
    let token = access_token()
        .await
        .map_err(|m| SubmitError::Retryable(m.to_string()))?;

    let body = serde_json::json!({
        "sender_batch_header": {
            "sender_batch_id": sender_batch_id,
            "email_subject": "Your PrimeLendRow loan",
            "email_message": "Your loan has been sent to your PayPal account.",
        },
        "items": [{
            // PAYPAL_ID, not EMAIL: the destination is the account PayPal
            // itself identified when the member connected, so there is no
            // address for anyone to mistype or tamper with.
            "recipient_type": "PAYPAL_ID",
            "receiver": payer_id,
            "amount": { "currency": "PHP", "value": format_centavos(centavos) },
            "note": note,
            "sender_item_id": sender_batch_id,
        }],
    });

    let res = http()
        .post(format!("{}/v1/payments/payouts", api_base()))
        .bearer_auth(&token)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("paypal payout submit: {e}");
            SubmitError::Retryable("PayPal was unreachable".to_string())
        })?;

    let status = res.status();
    if status.is_success() {
        let batch: PayoutBatchResponse = res.json().await.map_err(|e| {
            // The batch may well have been created; treating this as
            // retryable is safe because the batch id is already spent.
            tracing::error!("paypal payout body: {e}");
            SubmitError::Retryable("PayPal sent an unreadable reply".to_string())
        })?;
        tracing::info!(
            batch = %batch.batch_header.payout_batch_id,
            status = %batch.batch_header.batch_status,
            "payout submitted"
        );
        return Ok(batch.batch_header.payout_batch_id);
    }

    let detail = res.text().await.unwrap_or_default();
    if detail.contains("BATCH_ID_ALREADY_EXISTS") || detail.contains("DUPLICATE") {
        tracing::warn!(sender_batch_id, "paypal already has this payout batch");
        return Err(SubmitError::AlreadySubmitted);
    }
    tracing::error!("paypal payout status {status}: {detail}");
    // 5xx and 429 are the provider's problem and will pass; 4xx is ours.
    if status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(SubmitError::Retryable(format!("PayPal returned {status}")));
    }
    Err(SubmitError::Refused(format!("PayPal refused the payout ({status})")))
}

/// The latest word on a submitted batch.
pub async fn payout_status(batch_id: &str) -> Result<PayoutOutcome, &'static str> {
    if batch_id.is_empty() || batch_id.len() > 64 {
        return Err("Invalid batch reference");
    }
    let token = access_token().await?;
    let res = http()
        .get(format!("{}/v1/payments/payouts/{batch_id}", api_base()))
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("paypal payout status: {e}");
            "PayPal was unreachable"
        })?;
    if !res.status().is_success() {
        tracing::error!("paypal payout status {}", res.status());
        return Err("PayPal was unreachable");
    }
    let batch: PayoutBatchResponse = res.json().await.map_err(|e| {
        tracing::error!("paypal payout status body: {e}");
        "PayPal was unreachable"
    })?;

    // One item per batch by construction, so the item's fate is the batch's.
    let item = batch.items.into_iter().next();
    let item_id = item.as_ref().and_then(|i| i.payout_item_id.clone());
    let transaction_id = item.as_ref().and_then(|i| i.transaction_id.clone());
    let reason = item
        .as_ref()
        .and_then(|i| i.errors.as_ref())
        .map(|e| {
            format!(
                "{}: {}",
                e.name.as_deref().unwrap_or("error"),
                e.message.as_deref().unwrap_or("")
            )
        })
        .unwrap_or_else(|| batch.batch_header.batch_status.clone());

    let item_status = item
        .as_ref()
        .and_then(|i| i.transaction_status.as_deref())
        .unwrap_or(batch.batch_header.batch_status.as_str());

    Ok(match item_status {
        "SUCCESS" => match item_id {
            Some(item_id) => PayoutOutcome::Paid { item_id, transaction_id },
            // Paid without an item id is not something to post the books on.
            None => PayoutOutcome::Pending { item_id: None },
        },
        "UNCLAIMED" | "ONHOLD" => PayoutOutcome::Unclaimed { item_id },
        "RETURNED" | "REVERSED" | "REFUNDED" => PayoutOutcome::Returned { item_id, reason },
        "FAILED" | "BLOCKED" | "DENIED" => PayoutOutcome::Failed { reason },
        // PENDING, PROCESSING, ACCEPTED, and anything PayPal adds later.
        _ => PayoutOutcome::Pending { item_id },
    })
}

#[cfg(test)]
mod tests {
    use super::{format_centavos, parse_centavos};

    #[test]
    fn formats_centavos_back_without_floats() {
        assert_eq!(format_centavos(150000), "1500.00");
        assert_eq!(format_centavos(5), "0.05");
        assert_eq!(format_centavos(700), "7.00");
        assert_eq!(format_centavos(1), "0.01");
        // and round-trips through the parser it is the inverse of
        for c in [1i64, 5, 99, 100, 700, 150000, 999_999_99] {
            assert_eq!(parse_centavos(&format_centavos(c)), Ok(c));
        }
    }

    #[test]
    fn parses_paypal_amounts_without_floats() {
        assert_eq!(parse_centavos("1500.00"), Ok(150000));
        assert_eq!(parse_centavos("0.05"), Ok(5));
        assert_eq!(parse_centavos("7"), Ok(700));
        assert_eq!(parse_centavos("7.5"), Ok(750));
        assert!(parse_centavos("1.234").is_err());
        assert!(parse_centavos("-5.00").is_err());
        assert!(parse_centavos("").is_err());
        assert!(parse_centavos("1e3").is_err());
    }
}
