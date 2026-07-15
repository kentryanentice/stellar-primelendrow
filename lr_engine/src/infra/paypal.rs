//! Server-side PayPal order capture — the PHP money-in rail.
//!
//! The frontend renders PayPal Buttons with the *public* client id and hands
//! the engine nothing but an order id. Everything that decides money —
//! capturing the order, checking status/currency/amount — happens here with
//! the client SECRET, which never leaves the backend. The client's claimed
//! amount is never used: the centavos credited are whatever PayPal says was
//! actually captured.
//!
//! Env: PAYPAL_CLIENT_ID, PAYPAL_SECRET, PAYPAL_ENV ("live" | anything else
//! = sandbox). Fails closed: unconfigured means every capture is refused.

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

fn credentials() -> Option<(String, String)> {
    let id = std::env::var("PAYPAL_CLIENT_ID").ok()?;
    let secret = std::env::var("PAYPAL_SECRET").ok()?;
    if id.is_empty() || secret.is_empty() {
        return None;
    }
    Some((id, secret))
}

pub fn is_configured() -> bool {
    credentials().is_some()
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

#[cfg(test)]
mod tests {
    use super::parse_centavos;

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
