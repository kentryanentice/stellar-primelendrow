# Collateral Vault (Soroban)

Holds native XLM locked against PrimeLendRow's `xlm_collateral` loans, keyed
by the loan UUID's 16 bytes. Users can only ever put coins **in** (`lock`);
`release` and `seize` require the admin key — the engine's Stellar account —
so every exit from the vault is backend gated. `release` always pays the
depositor recorded at lock time; the admin cannot redirect it.

## Test

```sh
cargo test
```

Tests live in `src/test.rs` (kept out of `lib.rs`).

(The lockfile pins the transitive `ed25519-dalek` at 2.2.0 — 3.0.0 breaks
`soroban-env-host`'s testutils. Don't `cargo update` it past 2.x until
upstream catches up. Still required on soroban-sdk 27.)

## Build & deploy (testnet)

Requires the [stellar CLI](https://developers.stellar.org/docs/tools/cli):

```sh
rustup target add wasm32v1-none
stellar contract build

# the engine's admin identity (fund it on testnet with friendbot)
stellar keys generate lr-admin --network testnet --fund

stellar contract deploy \
  --wasm target/wasm32v1-none/release/collateral_vault.wasm \
  --source lr-admin --network testnet
# -> prints the contract id: C...

# native XLM's Stellar Asset Contract id on this network
stellar contract asset id --asset native --network testnet

stellar contract invoke --id <CONTRACT_ID> --source lr-admin --network testnet \
  -- initialize --admin <LR_ADMIN_G_ADDRESS> --token <NATIVE_SAC_C_ADDRESS>
```

## Wire it to the stack

`lr_engine` (.env / env.yaml):

```
COLLATERAL_CONTRACT_ID=C...          # the deployed vault; unset = XLM loans refused
HORIZON_URL=https://horizon-testnet.stellar.org
PAYPAL_CLIENT_ID=...                 # PHP rail (deposits + repayments)
PAYPAL_SECRET=...
PAYPAL_ENV=sandbox                   # "live" in production
```

`lr_frontend` (.env, optional — these are the defaults):

```
VITE_SOROBAN_RPC_URL=https://soroban-testnet.stellar.org
VITE_STELLAR_NETWORK=testnet         # "public" for mainnet
```

## Release / seize runbook (admin key only)

The engine queues on-chain work in `public.collateral_actions` when a loan
closes (release) or defaults (seize). Execute with the admin key and mark the
row done with the tx hash:

```sh
# loan repaid -> coins go home to the depositor automatically
stellar contract invoke --id <CONTRACT_ID> --source lr-admin --network testnet \
  -- release --loan_id <LOAN_UUID_AS_32_HEX_CHARS>

# default -> coins to the platform treasury
stellar contract invoke --id <CONTRACT_ID> --source lr-admin --network testnet \
  -- seize --loan_id <LOAN_UUID_AS_32_HEX_CHARS> --to <TREASURY_G_ADDRESS>
```

`loan_id` is the loan UUID with the dashes removed (16 bytes hex).
