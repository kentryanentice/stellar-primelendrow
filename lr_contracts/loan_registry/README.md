# Loan Registry (Soroban)

A tamper-evident, on-chain mirror of PrimeLendRow's `loans` table
(migration 020), keyed by each loan's UUID (16 bytes). It does **not** move
money — it records the loan's pinned terms and its status transitions on
Stellar, so a loan's whole life is auditable on a ledger nobody can edit.

Same trust model as the [collateral vault](../collateral_vault): only the
**admin** — the engine's Stellar account — can write; reads are open.

## Enforced invariants

- **Terms are pinned at birth** — `rate_bps` (monthly, 1–2000) and
  `policy_version` are set on `record_loan` and never rewritten (D8).
- **One open loan per borrower** — a borrower with a `pending` or `active`
  loan cannot open another; the slot frees only on a terminal status. Mirrors
  the engine's partial unique index.
- Amounts are whole centavos; `principal_outstanding` only ever decreases, and
  a loan cannot `close` while any principal remains.

## Lifecycle

```
record_loan ─▶ Pending ─ activate ─▶ Active ─ reduce_principal* ─▶ close ─▶ Closed
                 │                       └────────────────────── mark_defaulted ─▶ Defaulted
                 ├─ decline ─▶ Declined
                 └─ cancel  ─▶ Cancelled
```

| Function | Auth | Purpose |
|---|---|---|
| `initialize(admin)` | once | set the engine's account as the only writer |
| `record_loan(loan_id, borrower_id, product, principal, rate_bps, term_months, policy_version)` | admin | record a `pending` loan |
| `activate(loan_id, disbursed_at)` | admin | disburse: `pending` → `active`, set outstanding |
| `reduce_principal(loan_id, amount)` | admin | apply a principal repayment |
| `close(loan_id, closed_at)` | admin | close a fully-repaid loan |
| `mark_defaulted(loan_id, closed_at)` | admin | default an active loan |
| `decline(loan_id)` / `cancel(loan_id)` | admin | terminate a pending loan |
| `get_loan(loan_id)` / `open_loan_of(borrower_id)` | open | reconciliation reads |

## Test

```sh
cargo test
```

Tests live in `src/test.rs` (kept out of `lib.rs`).

## Build & deploy (testnet)

```sh
rustup target add wasm32v1-none
stellar contract build
stellar contract deploy \
  --wasm target/wasm32v1-none/release/loan_registry.wasm \
  --source lr-admin --network testnet
stellar contract invoke --id <CONTRACT_ID> --source lr-admin --network testnet \
  -- initialize --admin <LR_ADMIN_G_ADDRESS>
```

`lr_engine` (.env / env.yaml):

```
LOAN_REGISTRY_CONTRACT_ID=C...        # unset = loan anchoring skipped
```
