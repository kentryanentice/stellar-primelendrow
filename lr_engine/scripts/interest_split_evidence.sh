#!/usr/bin/env bash
#
# Evidence for SOW deliverable 3 — the interest split.
#
# Runs the allocation engine's own tests and saves what they print, so the
# reviewer gets the balance proof as a file rather than a screenshot:
#
#   * the published worked example at every guarantor tier, to the centavo;
#   * repayments whose interest does not divide evenly;
#   * the same payment split a thousand times, identically;
#   * 100,000 random repayments, every one summing back to the interest.
#
# The remaining acceptance criterion — at least ten repayments RECORDED with
# their splits — is evidence from a live run: make the repayments, then export
# them from the admin Interest history.
#
# Usage, from lr_engine/:
#
#   ./scripts/interest_split_evidence.sh            # print and save
#   ./scripts/interest_split_evidence.sh out.txt    # save somewhere else
#
# Exits non-zero if any split fails to balance, so it can gate CI.

set -euo pipefail
cd "$(dirname "$0")/.."

OUT="${1:-evidence/interest-split-$(date +%Y%m%d-%H%M%S).txt}"
mkdir -p "$(dirname "$OUT")"

# SQLX_OFFLINE: the allocation engine is pure arithmetic and needs no database,
# but the crate around it compiles against sqlx's offline cache.
{
    echo "PrimeLendRow — interest split evidence"
    echo "Generated: $(date -u '+%Y-%m-%d %H:%M:%S UTC')"
    echo "Commit:    $(git rev-parse --short HEAD 2>/dev/null || echo 'not a git checkout')"
    echo
    SQLX_OFFLINE=true cargo test --quiet interest_split_evidence -- --nocapture
    echo
    echo "---- every other test of the allocation engine ----"
    SQLX_OFFLINE=true cargo test --quiet domain
} 2>&1 | tee "$OUT"

echo
echo "Saved to $OUT"
