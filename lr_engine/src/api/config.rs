//! GET /config — the public identifiers the web app needs at runtime.
//!
//! These used to be baked into the frontend build as VITE_ variables. Serving
//! them from here means one place to set them, and changing one no longer
//! needs a frontend rebuild — the PayPal client id in particular is now
//! guaranteed to be the same one the engine itself uses with PayPal.
//!
//! **Only public values belong here.** Everything this returns reaches every
//! visitor's browser, exactly as the VITE_ variables did — the browser needs
//! them to load PayPal's buttons and to reach WalletConnect's relay. Secrets
//! (PAYPAL_SECRET, STRIPE_SECRET_KEY, GOOGLE_CLIENT_SECRET, …) stay out.
//!
//! No session required: the KYC wizard needs WalletConnect before an account
//! is approved.
//!
//! Env: PAYPAL_CLIENT_ID (already the engine's own PayPal credential),
//! WALLETCONNECT_PROJECT_ID (the Reown project id; unset leaves mobile wallets
//! on the browser-extension path only, as before).

use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct PublicConfig {
    pub paypal_client_id: Option<String>,
    pub walletconnect_project_id: Option<String>,
}

fn public_env(name: &str) -> Option<String> {
    std::env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

pub async fn public_config() -> Json<PublicConfig> {
    Json(PublicConfig {
        paypal_client_id: public_env("PAYPAL_CLIENT_ID"),
        walletconnect_project_id: public_env("WALLETCONNECT_PROJECT_ID"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A guard for the rule above: this response is public, so a field whose
    /// name suggests a secret must never appear in it.
    #[test]
    fn serves_no_secret_looking_field() {
        let json = serde_json::to_value(PublicConfig { paypal_client_id: None, walletconnect_project_id: None }).unwrap();
        for key in json.as_object().unwrap().keys() {
            assert!(!key.contains("secret") && !key.contains("private"), "public config exposes `{key}`");
        }
    }
}
