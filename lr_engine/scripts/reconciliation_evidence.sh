#!/usr/bin/env bash
#
# Evidence for SOW deliverable 1 — the zero-drift reconciliation report.
#
# Sets every XLM position the database recorded against the chain, both ways,
# and saves what it prints: each recorded lock, release, seizure and mark
# verified again on Horizon, and every movement the vault published matched
# back to a recorded transaction. What exactly is checked, and why, is in
# src/api/lending/drift.rs.
#
# Read-only. It writes nothing to the database or the chain, and starts no
# server and no sweeps. It needs the engine's environment (.env): DATABASE_URL,
# COLLATERAL_CONTRACT_ID and TREASURY_ADDRESS; HORIZON_URL and SOROBAN_RPC_URL
# default to testnet.
#
# The chain-to-database half covers what the RPC still holds — about a week —
# so run it at sprint close, and at least weekly before that.
#
# Usage, from lr_engine/:
#
#   ./scripts/reconciliation_evidence.sh            # print and save
#   ./scripts/reconciliation_evidence.sh out.txt    # save somewhere else
#
# Exits 0 on zero drift, 1 if anything disagrees, 2 if it could not finish.

set -uo pipefail
cd "$(dirname "$0")/.."

OUT="${1:-evidence/reconciliation-$(date +%Y%m%d-%H%M%S).txt}"
mkdir -p "$(dirname "$OUT")"

# SQLX_OFFLINE: the queries compile against the offline cache; the report
# itself reads the live database named in .env.
{
    echo "Commit:    $(git rev-parse --short HEAD 2>/dev/null || echo 'not a git checkout')"
    SQLX_OFFLINE=true cargo run --quiet -- reconcile
} 2>&1 | tee "$OUT"
status=${PIPESTATUS[0]}

echo
echo "Saved to $OUT"
exit "$status"
