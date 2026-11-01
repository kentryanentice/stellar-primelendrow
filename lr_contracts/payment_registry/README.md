# Payment Registry (Soroban)

A tamper-evident, on-chain mirror of PrimeLendRow's `loan_payments` table
(migration 023): one record per captured PayPal repayment, carrying the
engine's interest/principal split. It does **not** move money — the peso
capture happens at PayPal and the Postgres ledger is the source of truth. This
contract puts the payment *history* on Stellar, where it cannot be rewritten.

Same trust model as the [collateral vault](../collateral_vault): only the
**admin** — the engine's Stellar account — can write; reads are open.

## Enforced properties

- **Append-only** — there is no update or delete function. A recorded payment
  is permanent (same guarantee as the table's append-only trigger).
- **Deduplicated by capture id** — each record is keyed by its PayPal
  `rail_ref`, so a resent capture callback cannot double-log a payment. Mirrors
  the table's `UNIQUE (rail_ref)` index.
- Amounts are whole centavos; `amount_received > 0`, split parts `>= 0`.

## Interface

| Function | Auth | Purpose |
|---|---|---|
| `initialize(admin)` | once | set the engine's account as the only writer |
| `record_payment(rail_ref, loan_id, user_id, amount_received, interest_paid, principal_paid, excess, paid_at)` | admin | record one captured repayment |
| `get_payment(rail_ref)` | open | read a payment by capture id |
| `record_payments(payments)` | admin | atomically record 1–20 `PaymentEntry` values |
| `get_payments(rail_refs)` | open | read up to 20 payments, preserving order and missing entries |
| `get_admin()` | open | inspect the configured writer |

Batch entries contain the same fields as `record_payment`. Each keeps its own
capture-reference key and emits its own `PaymentRecorded` event. Duplicate or
invalid entries reject the entire batch. A GCYR signature on a batch transaction
authenticates every record in that transaction. The terminal backfill uses ten
entries per transaction and verifies every stored field and signed envelope.

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
  --wasm target/wasm32v1-none/release/payment_registry.wasm \
  --source lr-admin --network testnet
stellar contract invoke --id <CONTRACT_ID> --source lr-admin --network testnet \
  -- initialize --admin <LR_ADMIN_G_ADDRESS>
```

`lr_engine` (.env / env.yaml):

```
PAYMENT_REGISTRY_CONTRACT_ID=C...     # unset = payment anchoring skipped
```
