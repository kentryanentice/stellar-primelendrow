//! The zero-drift reconciliation report (SOW §4.1, deliverable 1): what the
//! database says every XLM position did, set against what the chain says.
//!
//! Run as `lr_engine reconcile` (see `engine.rs`), or through
//! `scripts/reconciliation_evidence.sh` to keep the output. Read-only on both
//! sides — it selects from the database and reads Horizon and the Soroban RPC —
//! and it starts nothing else: no server, no sweeps.
//!
//! Two directions, because drift can run either way:
//!
//!   database → chain  Every hash the database recorded is fetched again and
//!                     verified the way it was at confirm time: the lock moved
//!                     the recorded stroops from the recorded wallet into the
//!                     vault, a release went back to that wallet, a seizure
//!                     went to the treasury, a mark called the function it
//!                     names. Each position's status must also agree with the
//!                     movements recorded against it.
//!   chain → database  Every movement event the vault published must carry a
//!                     hash the database recorded. One that does not is coins
//!                     moved on chain that the books never heard of — a lock
//!                     whose confirm never arrived, or a release or seizure
//!                     signed outside the engine.
//!
//! The second direction is bounded by the RPC: a public node keeps about a week
//! of events, so the report states the window it covered and "zero drift" means
//! zero inside it. Run it at least weekly, and at sprint close for the evidence.
//!
//! Exit status: 0 zero drift, 1 drift found, 2 the check could not finish — a
//! network or database failure is not a finding either way.

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::infra::stellar;

/// id, loan id, wallet, locked stroops, lock hash, status
type PositionRow = (Uuid, Uuid, Option<String>, i64, Option<String>, String);
/// position id, action, status, tx hash, moved stroops
type ActionRow = (Uuid, String, String, Option<String>, Option<i64>);

/// The one verification answer that means "could not ask" rather than "the
/// chain disagrees" (see `infra::stellar`). Reporting either would be a guess,
/// so the report stops instead.
const UNREACHABLE: &str = "Blockchain network unreachable";

/// The movements a vault event can record; `configured` is the vault's own
/// setup, which no loan position accounts for.
const MOVEMENTS: [&str; 4] = ["locked", "recorded", "released", "seized"];

enum Check {
    Verified(String),
    Problem(String),
}

fn xlm(stroops: i64) -> String {
    format!("{}.{:07} XLM", stroops / 10_000_000, stroops % 10_000_000)
}

fn short(value: &str) -> &str {
    &value[..value.len().min(10)]
}

/// A loan the way the public records name it: the first block of its reference.
fn loan_ref(loan_id: &Uuid) -> String {
    loan_id.to_string()[..8].to_uppercase()
}

fn when(epoch: i64) -> String {
    DateTime::<Utc>::from_timestamp(epoch, 0)
        .map_or_else(|| "?".to_string(), |t| t.format("%Y-%m-%d %H:%M UTC").to_string())
}

/// A verification answer as a line of the report. `Err` only for an unreachable
/// network — every other refusal is the chain disagreeing, which is a finding.
fn judged(result: Result<Check, &'static str>, what: &str, hash: &str) -> Result<Check, String> {
    match result {
        Ok(check) => Ok(check),
        Err(UNREACHABLE) => Err(UNREACHABLE.to_string()),
        Err(why) => Ok(Check::Problem(format!("{what} {}: {why}", short(hash)))),
    }
}

async fn check_lock(hash: &str, wallet: &str, locked: i64, contract: &str) -> Result<Check, String> {
    let result = stellar::verify_collateral_lock(hash, wallet, contract).await.map(|moved| {
        if moved == locked {
            Check::Verified(format!("lock        {}  {} into the vault", short(hash), xlm(moved)))
        } else {
            Check::Problem(format!("lock {} moved {}, the database says {}", short(hash), xlm(moved), xlm(locked)))
        }
    });
    judged(result, "lock", hash)
}

async fn check_movement(
    action: &str,
    hash: &str,
    moved: Option<i64>,
    depositor: Option<&str>,
    treasury: Option<&str>,
    contract: &str,
) -> Result<Check, String> {
    let result = match action {
        "release" | "seize" => stellar::verify_vault_movement(hash, contract).await.map(|m| {
            let (expected, whose) = if action == "release" { (depositor, "depositor") } else { (treasury, "treasury") };
            if let Some(recorded) = moved.filter(|s| *s != m.stroops) {
                Check::Problem(format!("{action} {} moved {}, the database says {}", short(hash), xlm(m.stroops), xlm(recorded)))
            } else if expected.is_some_and(|to| to != m.to) {
                Check::Problem(format!("{action} {} went to {}…, not the {whose}", short(hash), short(&m.to)))
            } else {
                Check::Verified(format!("{action:<11} {}  {} out of the vault", short(hash), xlm(m.stroops)))
            }
        }),
        _ => stellar::verify_contract_call(hash, action)
            .await
            .map(|()| Check::Verified(format!("{action:<11} {}  recorded on chain", short(hash)))),
    };
    judged(result, action, hash)
}

pub async fn report(pool: &PgPool) -> i32 {
    match run(pool).await {
        Ok(0) => 0,
        Ok(_) => 1,
        Err(why) => {
            println!();
            println!("INCOMPLETE — {why}. Nothing above is a finding either way.");
            2
        }
    }
}

async fn run(pool: &PgPool) -> Result<usize, String> {
    let contract = stellar::contract_id().ok_or("COLLATERAL_CONTRACT_ID is not set")?;
    // Without it a seizure's destination cannot be checked, which the report
    // says rather than passing it silently.
    let treasury = stellar::treasury_address().ok();

    println!("PrimeLendRow — collateral reconciliation, database against chain");
    println!("Generated: {}", Utc::now().format("%Y-%m-%d %H:%M:%S UTC"));
    println!("Vault:     {contract}");
    if treasury.is_none() {
        println!("Treasury:  not configured — seizure destinations are not checked");
    }

    let positions: Vec<PositionRow> = sqlx::query_as(
        "SELECT id, loan_id, wallet_address, locked_stroops, lock_tx_hash, status
           FROM public.xlm_collateral ORDER BY created_at, id",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("the database could not be read ({e})"))?;
    let actions: Vec<ActionRow> = sqlx::query_as(
        "SELECT collateral_id, action, status, tx_hash, moved_stroops
           FROM public.collateral_actions ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("the database could not be read ({e})"))?;

    let mut findings: Vec<String> = Vec::new();
    let mut recorded: HashSet<&str> = HashSet::new();
    let mut verified = 0usize;

    println!();
    println!("== Database → chain: every recorded movement, verified again");
    for (id, loan_id, wallet, locked, lock_hash, status) in &positions {
        let reference = loan_ref(loan_id);
        println!();
        println!("Loan {reference}  {status}");
        let mut problems: Vec<String> = Vec::new();
        let note = |check: Check, verified: &mut usize, problems: &mut Vec<String>| match check {
            Check::Verified(line) => {
                *verified += 1;
                println!("  ✓ {line}");
            }
            Check::Problem(why) => {
                println!("  ✗ {why}");
                problems.push(why);
            }
        };

        match (lock_hash, wallet) {
            (Some(hash), Some(wallet)) => {
                recorded.insert(hash.as_str());
                note(check_lock(hash, wallet, *locked, &contract).await?, &mut verified, &mut problems);
            }
            (Some(hash), None) => note(
                Check::Problem(format!("lock {} is recorded with no wallet to check it against", short(hash))),
                &mut verified,
                &mut problems,
            ),
            (None, _) if matches!(status.as_str(), "locked" | "released" | "seized") => note(
                Check::Problem(format!("{status}, but no lock was ever recorded")),
                &mut verified,
                &mut problems,
            ),
            (None, _) => println!("  – nothing recorded on chain"),
        }

        let (mut releases, mut seizures, mut done) = (0, 0, 0);
        for (_, action, action_status, hash, moved) in actions.iter().filter(|a| a.0 == *id) {
            if action_status != "done" {
                println!("  – {action} waiting to be signed");
                continue;
            }
            done += 1;
            match action.as_str() {
                "release" => releases += 1,
                "seize" => seizures += 1,
                _ => {}
            }
            let Some(hash) = hash else {
                note(
                    Check::Problem(format!("{action} is marked done with no transaction recorded")),
                    &mut verified,
                    &mut problems,
                );
                continue;
            };
            recorded.insert(hash.as_str());
            let check = check_movement(action, hash, *moved, wallet.as_deref(), treasury.as_deref(), &contract).await?;
            note(check, &mut verified, &mut problems);
        }

        let consistent = match status.as_str() {
            "locked" => releases == 0 && seizures == 0,
            "released" => releases == 1 && seizures == 0,
            "seized" => seizures == 1 && releases == 0,
            "pending" | "cancelled" => lock_hash.is_none() && done == 0,
            _ => false,
        };
        if !consistent {
            note(
                Check::Problem(format!(
                    "the position says {status}, but {releases} release(s) and {seizures} seizure(s) are recorded against it",
                )),
                &mut verified,
                &mut problems,
            );
        }
        findings.extend(problems.into_iter().map(|p| format!("Loan {reference}: {p}")));
    }

    println!();
    println!("== Chain → database: every movement the vault published");
    let scan = stellar::vault_events(&contract).await.map_err(|why| why.to_string())?;
    println!(
        "Window: ledgers {}–{}, {} to {} (the RPC keeps about a week)",
        scan.from_ledger,
        scan.to_ledger,
        when(scan.from_time),
        when(scan.to_time),
    );
    let movements: Vec<&stellar::VaultEvent> =
        scan.events.iter().filter(|e| MOVEMENTS.contains(&e.name.as_str())).collect();
    let mut matched = 0usize;
    for event in &movements {
        if recorded.contains(event.tx_hash.as_str()) {
            matched += 1;
            continue;
        }
        let loan = Uuid::parse_str(&event.loan_hex).map_or_else(|_| event.loan_hex.clone(), |id| loan_ref(&id));
        let amount = event.amount.map(|a| format!(", {}", xlm(a))).unwrap_or_default();
        let finding = format!(
            "Loan {loan}: {} on chain at {} (tx {}{amount}) is not in the database",
            event.name,
            event.closed_at,
            short(&event.tx_hash),
        );
        println!("  ✗ {finding}");
        findings.push(finding);
    }
    println!("{matched} of {} movement events match a recorded transaction", movements.len());

    println!();
    println!("== Result");
    println!(
        "{} positions, {verified} recorded transactions verified on chain, {matched} vault events matched",
        positions.len(),
    );
    if findings.is_empty() {
        println!("ZERO DRIFT — the database and the vault agree.");
    } else {
        println!("{} DISAGREEMENT(S):", findings.len());
        for finding in &findings {
            println!("  ✗ {finding}");
        }
    }
    Ok(findings.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amounts_read_as_xlm_to_the_stroop() {
        assert_eq!(xlm(5_489_478_500), "548.9478500 XLM");
        assert_eq!(xlm(1), "0.0000001 XLM");
    }

    #[test]
    fn a_loan_is_named_the_way_the_public_records_name_it() {
        let id = Uuid::parse_str("958177f02f7e4b20a3c5892097349930").unwrap();
        assert_eq!(loan_ref(&id), "958177F0");
    }
}
