# Collateral Vault (Soroban)

Holds native XLM locked against PrimeLendRow's `xlm_collateral` loans, keyed by
the loan UUID's 16 bytes.

Two things are enforced here rather than promised:

**Every exit is gated on a recorded outcome.**

```
lock ──▶ Active ──mark_repaid──▶ Repaid ────release──▶ depositor
           │
           └───mark_defaulted──▶ Defaulted ──seize──▶ treasury
```

Users can only ever put coins **in** (`lock`). `mark_repaid`, `mark_defaulted`,
`release` and `seize` require the admin key — the engine's Stellar account. No
call sequence takes coins out of an `Active` position, and no seizure is
possible without a default recorded on-chain ahead of it. `release` always pays
the depositor recorded at lock time; the admin cannot redirect it.

**Nothing is priced at a number nobody checked.** `lock` and `seize` carry the
engine's candidate rate, and the vault measures it against a public SEP-40
price feed (Reflector) before acting. Stale feed, out-of-band quote, or a peso
rate its own legs don't support: the transaction fails. There is no unchecked
path, so the vault fails closed — no usable feed means no lock and no seizure.

The bounds are contract constants, not settings (`src/oracle.rs`):

| Constant | Value | Meaning |
| --- | --- | --- |
| `MAX_PRICE_AGE_SECS` | 900 | a feed point older than 15 min cannot price anything |
| `MAX_DEVIATION_BPS` | 500 | the quote must sit within 5% of the feed |
| `MIN_COLLATERAL_RATIO_BPS` | 10 000 | the admin can never configure below 100% cover |

The live collateral ratio (12 000 bps = 120% this sprint) and the feed address
*are* admin-settable via `configure` — the ratio is a policy parameter and
Reflector's deployment can move — but no admin can widen the staleness window
or the deviation band, and positions keep the rate and ratio they were struck
at.

## What the contract does *not* claim

The admin decides *what* to record. The contract makes the record unavoidable,
ordered and public; it does not judge whether a default really happened. And
within the 5% band, the backend still chooses the exact number submitted — the
feed narrows that discretion rather than removing it. Only the XLM/USD leg is
checkable on-chain; the USD/PHP leg the peso rate is crossed through is
recorded but no Stellar feed can verify it. See `src/lib.rs` and `src/oracle.rs`
for the model in full.

## Test

```sh
cargo test
```

Tests live in `src/test.rs` (kept out of `lib.rs`) and run against a mock
SEP-40 feed, so the refusals are exercised rather than described:
release-while-open, seize-without-recorded-default, stale feed, out-of-band
quote, under-collateralization, and admin auth on every exit.

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
```

`initialize` needs the price feed as well. Take the oracle contract id from
[Reflector's published feeds](https://reflector.network) for the network you
are on, and use the **External CEXs & DEXs** one — it quotes assets against
**USD** as `Other(Symbol)`, so `XLM`. (The Stellar DEX feed is useless here:
its base *is* XLM.) On testnet that is
`CCYOZJCOPG34LLQQ7N24YXBM7LL62R7ONMZ3G6WZAAYPB5OYKOMJRN63`.

**Ask the feed before you initialize** — `decimals` and the timestamp unit are
per-deployment facts, not standards:

```sh
stellar contract invoke --id <ORACLE> --source lr-admin --network testnet -- decimals
stellar contract invoke --id <ORACLE> --source lr-admin --network testnet \
  -- lastprice --asset '{"Other":"XLM"}'
# -> {"price":"17728982305851","timestamp":1788228900}
```

A **10-digit** timestamp is Unix seconds, so `--feed_time_divisor 1`; a
13-digit one is milliseconds, so `1000`. The testnet CEX/DEX feed above
answers in seconds, on 5-minute boundaries. Get this wrong and nothing is
mispriced — every price simply looks decades stale and every lock is refused.
A null `lastprice`, or a timestamp already older than 15 minutes, means the
feed isn't live enough to lend against right now.

```sh
stellar contract invoke --id <CONTRACT_ID> --source lr-admin --network testnet \
  -- initialize \
  --admin <LR_ADMIN_G_ADDRESS> \
  --token <NATIVE_SAC_C_ADDRESS> \
  --oracle <REFLECTOR_CONTRACT_ID> \
  --asset '{"Other":"XLM"}' \
  --feed_time_divisor 1 \
  --collateral_ratio_bps 12000
```

Check what it came up with — this is also what a reviewer reads to confirm the
authorization model against the actual deployment:

```sh
stellar contract invoke --id <CONTRACT_ID> --source lr-admin --network testnet \
  -- get_config
stellar contract invoke --id <CONTRACT_ID> --source lr-admin --network testnet \
  -- get_admin
```

To follow a feed migration or recalibrate the ratio (admin only; already-locked
positions are unaffected):

```sh
stellar contract invoke --id <CONTRACT_ID> --source lr-admin --network testnet \
  -- configure --oracle <NEW_ORACLE> --asset '{"Other":"XLM"}' \
     --feed_time_divisor 1 --collateral_ratio_bps 13000
```

## Wire it to the stack

`lr_engine` (.env / env.yaml):

```
COLLATERAL_CONTRACT_ID=C...          # the deployed vault; unset = XLM loans refused
HORIZON_URL=https://horizon-testnet.stellar.org
XLM_PRICE_SOURCES=                   # optional: narrow the price feeds, e.g.
                                     # "coingecko,kraken,er-api". Unset = all six
                                     # (coingecko, binance, kraken, coinbase,
                                     # er-api, frankfurter). Two must agree
                                     # within 5% or issuance is refused — and at
                                     # least one CRYPTO venue must answer, since
                                     # the vault checks the XLM/USD leg and a
                                     # price it can't check is one it won't act on.
XLM_PRICE_TIMEOUT_SECS=8             # per-feed HTTP timeout, 1..30
PAYPAL_CLIENT_ID=...                 # PHP rail (deposits + repayments)
PAYPAL_SECRET=...
PAYPAL_ENV=sandbox                   # "live" in production
```

`lr_frontend` (.env, optional — these are the defaults):

```
VITE_SOROBAN_RPC_URL=https://soroban-testnet.stellar.org
VITE_STELLAR_NETWORK=testnet         # "public" for mainnet
```

The borrower's wallet calls `lock` directly (`src/functions/Lending/stellarLock.ts`),
submitting the quote the engine pinned at application time. The engine then
verifies the resulting transaction on Horizon before disbursing — it believes
the chain, never the client.

## Release / seize runbook (admin key only)

The engine queues on-chain work in `public.collateral_actions` when a loan
closes or defaults. **Order matters**: the outcome is recorded first, and the
contract refuses the money movement if it isn't. Execute with the admin key and
mark each row done with its transaction hash.

`loan_id` is the loan UUID with the dashes removed (16 bytes hex).

```sh
# --- repaid ---------------------------------------------------------------
stellar contract invoke --id <CONTRACT_ID> --source lr-admin --network testnet \
  -- mark_repaid --loan_id <LOAN_UUID_AS_32_HEX_CHARS>
stellar contract invoke --id <CONTRACT_ID> --source lr-admin --network testnet \
  -- release --loan_id <LOAN_UUID_AS_32_HEX_CHARS>

# --- defaulted ------------------------------------------------------------
stellar contract invoke --id <CONTRACT_ID> --source lr-admin --network testnet \
  -- mark_defaulted --loan_id <LOAN_UUID_AS_32_HEX_CHARS>
# the seizure is priced at the day's checked quote, not at issuance:
stellar contract invoke --id <CONTRACT_ID> --source lr-admin --network testnet \
  -- seize --loan_id <LOAN_UUID_AS_32_HEX_CHARS> --to <TREASURY_G_ADDRESS> \
     --quote '{"php_per_xlm_centavos":"1500","usd_per_xlm_e8":"30000000","php_per_usd_centavos":"5000"}'
```

The `Seized` event records what those coins were worth at that checked price —
the number that decides how much debt they cover before any guarantor is
charged.

## Reconciliation

`get_lock` returns the whole position — coins, recorded state and the time it
was recorded, the pinned quote, what the feed said when it was accepted, and
the ratio enforced — which is what the engine compares `public.xlm_collateral`
against, and what the public proof page renders.
