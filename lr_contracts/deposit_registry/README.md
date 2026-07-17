# Deposit Registry (Soroban)

A tamper-evident, on-chain mirror of PrimeLendRow's `deposits` table
(migration 019), keyed by each deposit's UUID (16 bytes). It does **not**
custody money — real pesos move through PayPal and the Postgres ledger is the
source of truth. This contract records *what the engine decided*, on a ledger
nobody can rewrite.

Same trust model as the [collateral vault](../collateral_vault): only the
**admin** — the engine's Stellar account — can write; reads are open.

## Model

Every lot carries exactly one badge, and the (badge ↔ backing_loan) pair is
enforced on every write:

| Badge | Backing a loan? | Meaning |
|---|---|---|
| `available` | no | free, withdrawable |
| `lent` | yes | funding an active loan |
| `collateral` | yes | backing the owner's own deposit-backed loan |
| `pledged` | yes | backing someone else's guarantor loan |
| `withdrawn` | no | terminal — left the pool |

Amounts are whole centavos, matching the engine's `BIGINT`.

## Interface

| Function | Auth | Purpose |
|---|---|---|
| `initialize(admin)` | once | set the engine's account as the only writer |
| `record_deposit(deposit_id, user_id, amount)` | admin | record a fresh `available` lot |
| `set_badge(deposit_id, badge, backing_loan)` | admin | move a lot's badge as it starts/stops backing a loan |
| `withdraw(deposit_id)` | admin | mark an `available` lot `withdrawn` |
| `get_deposit(deposit_id)` | open | read a lot |

## Test

```sh
cargo test
```

Tests live in `src/test.rs` (kept out of `lib.rs`).

## Build & deploy (testnet)

Requires the [stellar CLI](https://developers.stellar.org/docs/tools/cli):

```sh
rustup target add wasm32v1-none
stellar contract build

# reuse the engine's admin identity (same account that owns the vault)
stellar contract deploy \
  --wasm target/wasm32v1-none/release/deposit_registry.wasm \
  --source lr-admin --network testnet
# -> prints the contract id: C...

stellar contract invoke --id <CONTRACT_ID> --source lr-admin --network testnet \
  -- initialize --admin <LR_ADMIN_G_ADDRESS>
```

## Wire it to the stack

`lr_engine` (.env / env.yaml):

```
DEPOSIT_REGISTRY_CONTRACT_ID=C...     # unset = deposit anchoring skipped
```
