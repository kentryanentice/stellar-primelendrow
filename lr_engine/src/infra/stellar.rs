//! Horizon-side verification of on-chain collateral movements.
//!
//! Whoever signs — the borrower for a `lock`, the vault admin for a release or
//! a seizure — the engine never trusts the claim that it happened. It
//! re-derives the truth from Horizon: the transaction must exist, have
//! succeeded, and show the native-XLM transfer it is supposed to have made.
//! The amount recorded is whatever the chain says moved, never what the client
//! posted.
//!
//! `mark_repaid` and `mark_defaulted` move no coins, so there is no transfer to
//! check and Horizon's plain JSON cannot tell us which contract entry point a
//! successful `invoke_host_function` called. `verify_contract_call` therefore
//! checks only that a contract invocation succeeded — and that is enough,
//! because the CONTRACT enforces the ordering: `release` is refused unless the
//! loan was recorded repaid and `seize` unless a default was recorded, both
//! admin-only. A mark that never really happened cannot be followed by a
//! movement that does, so the pair verifies itself and the engine does not
//! have to take either step on trust.
//!
//! Env: HORIZON_URL (default testnet), COLLATERAL_CONTRACT_ID (C... address;
//! unset = XLM collateral loans are refused, failing closed like KYC storage).

use std::sync::OnceLock;

use base64::Engine as _;
use serde::{Deserialize, Deserializer};

/// Horizon writes `null` where a field has nothing to say — a contract call
/// that moved no coins carries `"asset_balance_changes": null`, not an empty
/// array and not an absent key. `#[serde(default)]` alone does NOT cover that:
/// it fills in a MISSING field, and an explicit null is a present one, so the
/// whole response fails to decode. Every optional field below goes through
/// this instead.
fn null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
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

/// Where seized collateral goes. Read from the environment rather than taken
/// from the request, so an admin signing a seizure cannot point the coins at
/// an address of their choosing — the same reason a release's destination is
/// the contract's business and not the caller's.
///
/// Returns the reason on failure rather than a bare `None`: "not configured"
/// is the wrong thing to say about a value that IS configured and wrong, and
/// an operator staring at a populated `.env` has no way to guess which.
pub fn treasury_address() -> Result<String, &'static str> {
    let value = std::env::var("TREASURY_ADDRESS")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .ok_or("No treasury address is configured — set TREASURY_ADDRESS to the Stellar account seized collateral should go to")?;

    // A Soroban `Address` is either an account (G) or a contract (C), and the
    // vault accepts both. A treasury contract is a legitimate choice; it just
    // has to be one that can move XLM out again.
    if value.len() != 56 || !(value.starts_with('G') || value.starts_with('C')) {
        return Err("TREASURY_ADDRESS isn't a Stellar address — it must be a 56-character G… account, or a C… contract that can hold and move XLM");
    }

    // The one destination that is never right. Coins sent to the vault arrive
    // with no `Lock` record behind them, and both ways out of that contract
    // require one — so they would be stranded there permanently.
    if contract_id().is_some_and(|vault| vault == value) {
        return Err("TREASURY_ADDRESS is the vault contract itself — seized coins sent there would have no lock record and could never be moved out again");
    }

    Ok(value)
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
    #[serde(default, deserialize_with = "null_default")]
    asset_balance_changes: Vec<BalanceChange>,
    /// The invocation's arguments as base64 XDR. The second one is the
    /// contract function's name — see `invoked_function`.
    #[serde(default, deserialize_with = "null_default")]
    parameters: Vec<Parameter>,
}

#[derive(Deserialize)]
struct Parameter {
    #[serde(rename = "type")]
    param_type: String,
    #[serde(default, deserialize_with = "null_default")]
    value: String,
}

/// Horizon decodes Soroban SAC movements into these for invoke_host_function
/// operations — which is what lets us verify the transfer from plain JSON
/// instead of parsing XDR.
#[derive(Deserialize)]
struct BalanceChange {
    #[serde(rename = "type")]
    change_type: String,
    asset_type: String,
    #[serde(default, deserialize_with = "null_default")]
    from: String,
    #[serde(default, deserialize_with = "null_default")]
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

/// Fetches a transaction's operations, having first established that the
/// transaction exists and succeeded. Shared by every verification below so
/// there is one description of "what Horizon said happened".
async fn succeeded_operations(tx_hash: &str) -> Result<Vec<Operation>, &'static str> {
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
        return Err("That transaction failed on-chain");
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
    Ok(ops.embedded.records)
}

/// Sums the native transfers in `ops` that match a direction, in stroops.
/// `to` of `None` means "anywhere" — used where the destination is the
/// treasury address the admin configured rather than one the engine pinned.
fn native_transfer(ops: &[Operation], from: &str, to: Option<&str>) -> Result<(i64, String), &'static str> {
    let mut total: i64 = 0;
    let mut destination = String::new();
    for op in ops {
        if op.op_type != "invoke_host_function" {
            continue;
        }
        for change in &op.asset_balance_changes {
            if change.change_type == "transfer"
                && change.asset_type == "native"
                && change.from == from
                && to.is_none_or(|expected| change.to == expected)
            {
                total = total
                    .checked_add(parse_stroops(&change.amount)?)
                    .ok_or("Invalid on-chain amount")?;
                destination = change.to.clone();
            }
        }
    }
    Ok((total, destination))
}

/// Verifies `tx_hash` locked native XLM from `expected_from` into
/// `expected_contract`, returning the stroops that actually moved.
pub async fn verify_collateral_lock(
    tx_hash: &str,
    expected_from: &str,
    expected_contract: &str,
) -> Result<i64, &'static str> {
    let ops = succeeded_operations(tx_hash).await?;
    // Sum every native transfer wallet -> vault in this tx (there is exactly
    // one in a normal lock; summing keeps a multi-op tx honest either way).
    let (total, _) = native_transfer(&ops, expected_from, Some(expected_contract))?;
    if total <= 0 {
        return Err("No collateral transfer from your wallet was found in that transaction");
    }
    Ok(total)
}

/// What a release or a seizure actually moved out of the vault.
pub struct VaultMovement {
    pub stroops: i64,
    /// Where the coins went — the depositor for a release (the contract picks
    /// it, the admin cannot redirect it), the treasury for a seizure.
    pub to: String,
}

/// Verifies `tx_hash` moved native XLM OUT of the vault, returning how much
/// and to whom. This is the direction that matters: the contract refuses to
/// release anywhere but the recorded depositor, so a transfer leaving the
/// vault in a successful transaction is a movement the contract itself
/// authorized.
pub async fn verify_vault_movement(
    tx_hash: &str,
    contract: &str,
) -> Result<VaultMovement, &'static str> {
    let ops = succeeded_operations(tx_hash).await?;
    let (stroops, to) = native_transfer(&ops, contract, None)?;
    if stroops <= 0 {
        return Err("That transaction moved no collateral out of the vault");
    }
    Ok(VaultMovement { stroops, to })
}

/// The contract function a `Sym` parameter names.
///
/// Horizon hands invocation arguments back as base64 XDR, and the entry point
/// is the `Sym` among them. Decoding one value is cheap enough to do by hand:
/// an `ScVal` is a 4-byte discriminant (15 = SCV_SYMBOL) followed by a 4-byte
/// length and the ASCII, padded to a 4-byte boundary. Pulling in an XDR
/// dependency to read eleven characters would cost more than it explains.
fn symbol_value(value: &str) -> Option<String> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(value).ok()?;
    if bytes.len() < 8 || u32::from_be_bytes(bytes[0..4].try_into().ok()?) != 15 {
        return None;
    }
    let len = u32::from_be_bytes(bytes[4..8].try_into().ok()?) as usize;
    let end = 8usize.checked_add(len)?;
    if end > bytes.len() {
        return None;
    }
    String::from_utf8(bytes[8..end].to_vec()).ok()
}

/// Verifies `tx_hash` successfully invoked `expected_fn` on a contract.
///
/// Used for the two movements that record an outcome without moving coins.
/// There is no transfer to check, so the entry point IS the check — which
/// matters most where an operator pastes a hash by hand: a real transaction
/// aimed at the wrong movement is a plausible mistake, and this is what
/// catches it. The contract's own ordering (no `release` without a recorded
/// repayment, no `seize` without a recorded default) is still what makes a
/// fabricated mark useless, since the movement after it would be refused.
pub async fn verify_contract_call(
    tx_hash: &str,
    expected_fn: &str,
) -> Result<(), &'static str> {
    let ops = succeeded_operations(tx_hash).await?;
    let invocations: Vec<&Operation> = ops
        .iter()
        .filter(|op| op.op_type == "invoke_host_function")
        .collect();
    if invocations.is_empty() {
        return Err("That transaction didn't invoke the vault contract");
    }

    // A named function among the invocation's arguments settles it. Horizon
    // occasionally returns parameters it can't decode; rather than accept a
    // transaction we couldn't read, say so.
    let called: Vec<String> = invocations
        .iter()
        .flat_map(|op| op.parameters.iter())
        .filter(|p| p.param_type == "Sym")
        .filter_map(|p| symbol_value(&p.value))
        .collect();
    if called.is_empty() {
        return Err("That transaction's contract call couldn't be read back from the network");
    }
    if !called.iter().any(|name| name == expected_fn) {
        return Err("That transaction called a different contract function than this movement");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{parse_stroops, symbol_value};

    #[test]
    fn reads_the_invoked_function_name() {
        // Exactly what Horizon returned for a mark_repaid invocation.
        assert_eq!(
            symbol_value("AAAADwAAAAttYXJrX3JlcGFpZAA="),
            Some("mark_repaid".to_string()),
        );
        // An Address parameter is discriminant 18, not 15 — not a symbol.
        assert_eq!(
            symbol_value("AAAAEgAAAAEihEBchmJNhlcOmiDyjY2FNSIwotabZ02K/Ftl/GMPaQ=="),
            None,
        );
        assert_eq!(symbol_value("not base64 at all"), None);
        // A length running past the end of the buffer must not panic.
        assert_eq!(symbol_value("AAAADwAAAH9hYg=="), None);
    }

    #[test]
    fn parses_xlm_amounts_to_stroops() {
        assert_eq!(parse_stroops("250.1234567"), Ok(2_501_234_567));
        assert_eq!(parse_stroops("1"), Ok(10_000_000));
        assert_eq!(parse_stroops("0.0000001"), Ok(1));
        assert!(parse_stroops("1.12345678").is_err());
        assert!(parse_stroops("-1").is_err());
    }
}
