//! Horizon-side verification of on-chain collateral locks.
//!
//! The frontend signs and submits the vault contract's `lock` transaction
//! itself (its wallet, its coins); the engine never trusts that claim. It
//! re-derives the truth from Horizon: the transaction must exist, have
//! succeeded, invoke OUR contract, and show a native-XLM transfer from the
//! caller's KYC-anchored wallet into the contract. The amount credited is
//! whatever the chain says moved — never what the client posted.
//!
//! Env: HORIZON_URL (default testnet), COLLATERAL_CONTRACT_ID (C... address;
//! unset = XLM collateral loans are refused, failing closed like KYC storage).

use std::sync::OnceLock;

use serde::Deserialize;

fn http() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .expect("reqwest client")
    })
}

fn horizon_base() -> String {
    std::env::var("HORIZON_URL")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "https://horizon-testnet.stellar.org".to_string())
}

/// The vault contract this deployment trusts. `None` = not configured.
pub fn contract_id() -> Option<String> {
    std::env::var("COLLATERAL_CONTRACT_ID")
        .ok()
        .filter(|v| v.len() == 56 && v.starts_with('C'))
}

#[derive(Deserialize)]
struct TxResponse {
    successful: bool,
}

#[derive(Deserialize)]
struct OperationsResponse {
    #[serde(rename = "_embedded")]
    embedded: Embedded,
}

#[derive(Deserialize)]
struct Embedded {
    records: Vec<Operation>,
}

#[derive(Deserialize)]
struct Operation {
    #[serde(rename = "type")]
    op_type: String,
    #[serde(default)]
    asset_balance_changes: Vec<BalanceChange>,
}

/// Horizon decodes Soroban SAC movements into these for invoke_host_function
/// operations — which is what lets us verify the transfer from plain JSON
/// instead of parsing XDR.
#[derive(Deserialize)]
struct BalanceChange {
    #[serde(rename = "type")]
    change_type: String,
    asset_type: String,
    #[serde(default)]
    from: String,
    #[serde(default)]
    to: String,
    amount: String,
}

/// "250.1234567" XLM -> stroops (1 XLM = 10^7 stroops), string math only.
fn parse_stroops(value: &str) -> Result<i64, &'static str> {
    let (whole, frac) = match value.split_once('.') {
        Some((w, f)) => (w, f),
        None => (value, ""),
    };
    if whole.is_empty() || whole.len() > 12 || !whole.chars().all(|c| c.is_ascii_digit()) {
        return Err("Invalid on-chain amount");
    }
    if frac.len() > 7 || !frac.chars().all(|c| c.is_ascii_digit()) {
        return Err("Invalid on-chain amount");
    }
    let whole: i64 = whole.parse().map_err(|_| "Invalid on-chain amount")?;
    let mut frac_val: i64 = 0;
    if !frac.is_empty() {
        frac_val = frac.parse().map_err(|_| "Invalid on-chain amount")?;
        frac_val *= 10i64.pow(7 - frac.len() as u32);
    }
    Ok(whole * 10_000_000 + frac_val)
}

/// Verifies `tx_hash` locked native XLM from `expected_from` into
/// `expected_contract`, returning the stroops that actually moved.
pub async fn verify_collateral_lock(
    tx_hash: &str,
    expected_from: &str,
    expected_contract: &str,
) -> Result<i64, &'static str> {
    if tx_hash.len() != 64 || !tx_hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("Invalid transaction hash");
    }
    let base = horizon_base();

    let tx = http()
        .get(format!("{base}/transactions/{tx_hash}"))
        .send()
        .await
        .map_err(|e| {
            tracing::error!("horizon tx fetch: {e}");
            "Blockchain network unreachable"
        })?;
    if tx.status() == reqwest::StatusCode::NOT_FOUND {
        return Err("Transaction not found on the network yet — try again shortly");
    }
    if !tx.status().is_success() {
        tracing::error!("horizon tx status {}", tx.status());
        return Err("Blockchain network unreachable");
    }
    let tx: TxResponse = tx.json().await.map_err(|e| {
        tracing::error!("horizon tx body: {e}");
        "Blockchain network unreachable"
    })?;
    if !tx.successful {
        return Err("The lock transaction failed on-chain");
    }

    let ops = http()
        .get(format!("{base}/transactions/{tx_hash}/operations?limit=50"))
        .send()
        .await
        .map_err(|e| {
            tracing::error!("horizon ops fetch: {e}");
            "Blockchain network unreachable"
        })?;
    if !ops.status().is_success() {
        tracing::error!("horizon ops status {}", ops.status());
        return Err("Blockchain network unreachable");
    }
    let ops: OperationsResponse = ops.json().await.map_err(|e| {
        tracing::error!("horizon ops body: {e}");
        "Blockchain network unreachable"
    })?;

    // Sum every native transfer wallet -> vault in this tx (there is exactly
    // one in a normal lock; summing keeps a multi-op tx honest either way).
    let mut total: i64 = 0;
    for op in ops.embedded.records {
        if op.op_type != "invoke_host_function" {
            continue;
        }
        for change in op.asset_balance_changes {
            if change.change_type == "transfer"
                && change.asset_type == "native"
                && change.from == expected_from
                && change.to == expected_contract
            {
                total = total
                    .checked_add(parse_stroops(&change.amount)?)
                    .ok_or("Invalid on-chain amount")?;
            }
        }
    }
    if total <= 0 {
        return Err("No collateral transfer from your wallet was found in that transaction");
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::parse_stroops;

    #[test]
    fn parses_xlm_amounts_to_stroops() {
        assert_eq!(parse_stroops("250.1234567"), Ok(2_501_234_567));
        assert_eq!(parse_stroops("1"), Ok(10_000_000));
        assert_eq!(parse_stroops("0.0000001"), Ok(1));
        assert!(parse_stroops("1.12345678").is_err());
        assert!(parse_stroops("-1").is_err());
    }
}
