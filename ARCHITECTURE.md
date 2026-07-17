# PrimeLendRow — Architecture

This document describes how PrimeLendRow is built to support the user journey in
[README.md](README.md). The README is the *workflow* (what a user does, step by
step); this is the *system* (the services, data, and rules that make that
workflow happen). Read them as a pair — every section here maps back to a
section there.

The guiding principle is simple: **the frontend is a window, not a calculator.**
It never computes money or decides eligibility; it renders what the engine
decided. Every state change — a deposit, a loan, a repayment, an admin action —
is validated and committed by the backend inside a single database transaction,
so the records never disagree with each other.

---

## 1. System overview

```
                        ┌──────────────────────────────────────┐
   Browser              │  lr_frontend  (React 19 + Vite)       │
   (the window)         │  · KYC OCR + liveness run client-side │
                        │  · Stellar wallet signing (Freighter) │
                        └───────────────┬──────────────────────┘
                                        │ HTTPS / JSON (cookie session)
                                        ▼
                        ┌──────────────────────────────────────┐
                        │  lr_engine  (Rust + Axum)  — the brain │
                        │  auth · KYC · wallets · credit ·       │
                        │  lending · ledger · anchoring          │
                        └───┬─────────┬─────────┬─────────┬─────┘
                            │         │         │         │
              PostgreSQL ◀──┘         │         │         └──▶ Resend  (email OTP)
        (notebook + books)            │         │                via lr-mailer Worker
                                      │         │
                     Stellar network ◀┘         └▶ PayPal API  (fiat deposit / repay rail)
             · Horizon (tx verify)
             · Soroban collateral_vault (XLM lock)
             · SHA-256 event anchoring

        Supabase Storage ◀── lr-cdn Worker ── private ID photos (HMAC-signed URLs)
```

**Components:**

| Service | Role | Stack |
|---|---|---|
| `lr_frontend` | The web app ("the window") — renders state, runs KYC capture, signs Stellar transactions | React 19, Vite, TypeScript (React Compiler), SCSS |
| `lr_engine` | The backend ("the brain") — all business rules, money math, and persistence | Rust, Axum, SQLx → PostgreSQL |
| `lr_api` | MongoDB Atlas port of the identity/auth + KYC layer (same design, document store) | Rust, Axum, MongoDB |
| `lr_contracts/collateral_vault` | Soroban smart contract holding XLM collateral, keyed by loan id | Rust, `soroban-sdk` |
| `lr-mailer` | Edge Worker that sends transactional email (OTP, resets) via Resend | Cloudflare Worker, TypeScript |
| `lr-cdn` | Edge Worker proxying Supabase Storage for private ID photos with caching | Cloudflare Worker, TypeScript |

`lr_engine` is the reference backend and the one the full lending workflow runs
on (Postgres). `lr_api` is a MongoDB-Atlas port of the identity and KYC layer,
kept in step with the engine's design for deployments that prefer a document
store.

> The core promise — *money, trust, and records change together or not at all* —
> is trivially true inside one Postgres transaction and genuinely hard across
> many services, which is why the engine stays a single program until measured
> operational pain justifies splitting it.

---

## 2. Repository layout

```
lr_frontend/     React app — pages/, elements/, functions/, providers/, scss/
lr_engine/       Rust engine — the primary backend
  src/api/         users/ · kyc/ · wallets/ · credit/ · lending/
  src/routes/      the HTTP surface (/api/v1)
  src/infra/       database, storage, mailer client, crypto helpers
  src/migrations/  the SQL schema, 001 … 024 (applied in order)
lr_api/          Rust engine — MongoDB Atlas port of the auth + KYC layer
lr_contracts/    Soroban contracts — collateral_vault (XLM lock)
lr-mailer/       Cloudflare Worker — Resend email sender
lr-cdn/          Cloudflare Worker — Supabase Storage proxy for ID photos
```

Inside `lr_engine/src/api/lending/` the responsibilities are split by job:
`pool` and `deposit`/`deposits_list`/`withdraw` (the pool), `lots` (deposit
lots), `quote`/`apply`/`collateral` (borrowing), `guarantors`, `repay`/`payments`
(repayment), `policy` (versioned rate & limit rules), `ledger` (the append-only
record), `domain` (pure money math), and `admin` (review actions).

---

## 3. The request model

Every request follows the same shape, and the money-moving ones all commit
inside **one** transaction:

```
   request arrives (cookie session → authenticated user)
         │
   1. AUTHORIZE   role / KYC gate — is this user allowed to do this at all?
   2. VALIDATE    pure rule functions in domain/ — eligibility, limits, math
   3. WRITE       one Postgres transaction:
                    · ledger entry (append-only)
                    · row updates (deposits, loans, wallets, …)
   4. RESPOND     JSON the window renders
```

If any step fails, the whole transaction rolls back — there is no moment where a
loan exists without its ledger entry, or a balance changes without a record of
why. Routes live under `/api/v1` so breaking changes can move to `/api/v2`
without stranding old clients.

---

## 4. Data model

The schema is built up across migrations `001`–`024`. Grouped by concern:

**Identity & sessions**
- `users` — account, role (`Pending` → `User` → admin), password hash (Argon2)
- `verification_codes` — email OTP for registration and re-verification
- `password_reset_codes` — reset flow
- `sessions`, `access_tokens` — authenticated sessions, keyed to a user
- `login_lockout` — throttles brute-force login attempts
- `used_nonces` — spent wallet-challenge nonces (replay protection)

**KYC**
- `kyc` — submission record: extracted fields, face-match result, connected
  wallet, status (`Not Submitted` → `Submitted` → `Under Review` → `Approved`/
  `Rejected`). ID photos are **not** in Postgres — they live in private Supabase
  Storage, referenced by HMAC-signed URLs (see §7).

**Trust & wallets**
- `credit_score` — the member's score (starts at 50) and its history
- `wallets` — connected Stellar wallets: address, optional label, source
  (KYC-verified / user-added), status (Active / Disconnected)

**Lending ledger**
- `ledger` — the append-only record of every financial event
- `lending_policy` — versioned rate & limit tiers (changing a rule is a new
  version; live loans keep the parameters they were issued under)
- `deposits` — pool deposits and their status (Available / Reserved / Locked)
- `loans` — loan records: amount, term, product, rate, schedule, status
- `guarantors` — guarantor invitations and their locked backing (cap of 2)
- `xlm_collateral` — the link between a loan and its on-chain XLM lock
- `loan_payments` — the diminishing-balance repayment schedule and payments

---

## 5. Subsystems

Each subsystem below implements the matching README section.

### 5.1 Identity & authentication → README §1–2

Registration takes email, username, and password; the password is hashed with
**Argon2**. An OTP is generated, sent through the `lr-mailer` Worker (Resend),
and must be verified before the account is created. New accounts start
`Pending` with credit score `50`, no wallet, and no KYC.

Login accepts email or username, issues a server-side **session** (HTTP-only
cookie), and is protected by `login_lockout` throttling. A `Pending` user can
view public pool stats and manage settings but cannot lend, borrow, or vouch
until KYC is approved — the gate is enforced server-side on every protected
route, never by the UI.

### 5.2 KYC & liveness → README §3

KYC capture runs **in the browser**, so raw biometric processing never touches
the server as a dependency:

- **OCR** — `tesseract.js` reads the uploaded government ID (PhilSys, driver's
  license, passport, postal ID) and extracts name, date of birth, ID number, and
  type for the user to review.
- **Liveness & face match** — `@vladmandic/human` runs a blink challenge with
  anti-spoof checks on the live camera feed, captures a frame on success, and
  matches it against the ID portrait (`@vladmandic/face-api`), producing a
  similarity score and match result.

The user connects a Stellar wallet as part of the wizard (this becomes the
**KYC-verified wallet**), then submits. The engine stores the extracted fields,
the match result, and the wallet address; the ID photo goes to private storage.
An **admin** reviews the submission (`kyc/admin`) and approves or rejects.
Approval flips the role to `User`; rejection returns the user to re-verification.

### 5.3 Wallet management → README §4

The KYC-verified wallet is active on approval. From Settings a user may connect
**additional** Stellar wallets (Freighter, Lobstr, Albedo, and others via
`@creit.tech/stellar-wallets-kit`), up to **5 active** at once.

Each connection is proven by a **challenge–response**, not a bare address: the
engine issues a random nonce, the wallet signs it, and the engine verifies the
Ed25519 signature (`ed25519-dalek`) against the Stellar address
(`stellar-strkey`). Spent nonces are recorded in `used_nonces` so a signature
can never be replayed. Disconnected wallets are retained as history, never
deleted.

### 5.4 The lending pool → README §5

Deposits enter the pool through **PayPal** (the fiat rail) and become
`Available`. Available funds can be withdrawn, or `Reserved` → `Locked` when
assigned to a loan or a guarantor pledge. Locked funds cannot be withdrawn,
borrowed against, or pledged again until the backed loan closes. Every movement
writes a `ledger` entry, and pool utilization is derived from the ledger, not
stored as a mutable number.

### 5.5 Borrowing & credit → README §6

Three products, distinguished by what backs them:

| Product | Collateral |
|---|---|
| **Deposit-backed** | the borrower's own pool deposit |
| **XLM-collateral** | the borrower's XLM, locked on-chain (§5.7) |
| **Guarantor** | a guarantor's deposit and/or XLM (§5.6) |

`quote` returns the member's eligibility from their `credit_score` tier in the
active `lending_policy` version — the rate and maximum loan size (from ₱5,000 at
2%/mo for the entry tier up to ₱100,000 at 1%/mo for the top tier). `apply`
re-validates everything server-side — role, KYC, score, limit, available
liquidity, and collateral coverage — before a loan can be created. The frontend
only displays these numbers; it never decides them.

### 5.6 Guarantors → README §7

A borrower may invite up to **two** guarantors (`024_guarantor_cap_two`). Each
receives a request they can accept or decline; on acceptance, the guarantor's
backing is `Locked` and stays locked until the loan closes (released) or
defaults (liquidated). Guarantor invitations and their state live in the
`guarantors` table.

### 5.7 XLM collateral — the Soroban vault → README §6, §8, §11

XLM-collateral loans are secured by an on-chain lock in the `collateral_vault`
Soroban contract, keyed by the loan's UUID (16 bytes). The security model is
deliberately asymmetric — **anyone can put funds in, only the engine can take
them out:**

- `lock` — the borrower signs from their own wallet; funds move into the vault.
  The engine verifies the resulting transaction on **Horizon** before the loan
  disburses.
- `release` — **admin-only**; on repayment, funds return to the depositor
  recorded at lock time. The destination cannot be redirected, so even a
  compromised engine key cannot route a release elsewhere.
- `seize` — **admin-only**; the default/liquidation path, sending funds to the
  treasury address.

One lock per loan (top-ups are refused so a single transaction hash maps to a
single position), and locks extend their ledger TTL on every touch so an active
loan's collateral cannot expire out of existence. The contract is built with
overflow checks left **on** in release — it holds user funds, so a silent wrap
is unacceptable.

### 5.8 Loan creation & repayment → README §8–10

On approval, collateral (and any pledges) are frozen, the loan's parameters are
pinned from the current policy version, the `loan_payments` schedule is
generated, and the rail disburses. Repayment uses a **diminishing-balance**
model — interest is charged on the remaining principal, so it shrinks as the
loan is paid down (e.g. a ₱1,000 / 3-month / 2%-per-month loan pays ₱20, then
₱13.33, then ₱6.67 in interest). Payments arrive through **PayPal**; each is
confirmed, posted to the ledger, and the outstanding balance is updated
immediately, split into principal and interest. On the final payment the loan
closes, collateral and pledges release (on-chain `release` for XLM), and only
**closed** loans are eligible for a credit-score increase.

### 5.9 Default handling → README §11

Missed obligations move a loan `Active` → `Overdue` → `Defaulted`. Recovery runs
in a fixed order and never reaches ordinary savers' deposits:

```
1. borrower deposit   2. borrower XLM (seize)   3. guarantor deposit   4. guarantor XLM (seize)
```

After liquidation the loan is `Closed` and credit penalties are applied.

### 5.10 Stellar anchoring → README §12

Financial events (deposit, withdrawal, loan creation, repayment, closure,
collateral lock/release, default) are anchored on Stellar as tamper-evidence.
The engine builds a canonical payload for the event, hashes it with **SHA-256**,
and submits that hash in a Stellar transaction; the confirmation is recorded
against the event. **Only the cryptographic proof is anchored** — no personal
data, KYC documents, ID numbers, or facial images ever go on-chain.

---

## 6. Security model

- **Passwords** — Argon2 hashing; never stored or logged in clear.
- **Sessions** — server-side sessions in HTTP-only cookies; `login_lockout`
  throttles brute force; `lr_api` additionally carries CSRF, rate-limiting, and
  login-guard middleware in `infra/`.
- **Encryption at rest** — AES-GCM (`aes-gcm`) for sensitive fields.
- **Wallet proof** — Ed25519 challenge–response with single-use nonces; no
  action trusts a self-reported address.
- **Private ID photos** — stored in a non-public Supabase bucket and served only
  through the `lr-cdn` Worker via **HMAC-signed, expiring URLs**; the bytes are
  never publicly listable.
- **On-chain custody** — the vault is backend-gated: `release`/`seize` are
  admin-only and the release destination is fixed at lock time, so funds can
  enter permissionlessly but only leave under engine authority.
- **Server-side authority** — every rule (eligibility, limits, money math) is
  enforced in the engine; the frontend cannot grant itself capability it wasn't
  given.

---

## 7. External integrations

| Integration | Used for | How |
|---|---|---|
| **PayPal** | Fiat rail for deposits and repayments | Backend verifies orders/captures over HTTPS (`reqwest`) before posting to the ledger |
| **Stellar Horizon** | Verifying on-chain locks; anchoring | Engine reads transactions to confirm a `lock` before disbursing, and submits anchoring transactions |
| **Soroban** | XLM collateral custody | `collateral_vault` contract (§5.7) |
| **Resend** (via `lr-mailer`) | Transactional email (OTP, resets) | Engine calls the Worker at `stellar.mailer.primelendrow.com` |
| **Supabase Storage** (via `lr-cdn`) | Private ID photo hosting | Worker at `cdn.primelendrow.com` proxies the bucket with edge caching |

---

## 8. Deployment

- **Frontend** — built with Vite (`tsc -b && vite build`) and deployed to
  **Cloudflare Pages** (`primelendrow`). A `build:lan` mode exists for testing
  KYC capture on a real phone over LAN.
- **Engine** — the Rust binary (`lr_engine`) against a **Supabase-hosted
  PostgreSQL** database; the schema is defined by the ordered migrations
  `001`–`024` in `src/migrations/`.
- **`lr_api`** — the same auth/KYC layer against **MongoDB Atlas**, for
  document-store deployments.
- **Contracts** — `collateral_vault` compiled to Wasm and deployed to Soroban;
  its admin is the engine's own Stellar account.
- **Edge Workers** — `lr-mailer` and `lr-cdn` deployed with Wrangler, each bound
  to its own custom domain so it can only ever serve its intended Worker.

---

## 9. Design rules that keep the system honest

- **One brain, one notebook.** The engine is the only writer of money and truth;
  the window only reads and displays.
- **All-or-nothing writes.** Every money movement is one Postgres transaction —
  ledger and rows commit together or not at all.
- **Rules are versioned data.** Rates, limits, and tiers live in
  `lending_policy` versions; live loans keep the parameters they were born with.
- **No floats near money.** Amounts are whole centavos with one documented
  rounding rule.
- **The frontend never computes.** Eligibility and balances come from the engine;
  `/api/v1` is the contract between them.
- **Everything specific is tunable.** The rate tiers, limits, and caps in this
  document are the current policy defaults, versioned in `lending_policy` — not
  constants baked into code.
