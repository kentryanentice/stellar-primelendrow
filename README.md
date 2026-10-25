# PrimeLendRow

PrimeLendRow is a lending platform for Filipino savers and borrowers. Verified members deposit into a shared pool, borrow against their savings or XLM, and invite guarantors when they need more than their own assets cover.

This README follows the Instawards Statement of Work (SOW) and the features already implemented. Validation uses **Stellar Testnet and sandbox payments**.

## How it works

```text
Register → Verify identity and connect a wallet → Deposit → Borrow → Repay → Close
```

1. **Create an account.** Register with an email, username and password, verify the email OTP, then sign in.
2. **Verify identity.** Upload a government ID, review the scanned details, complete the liveness and face check, and connect a Stellar wallet. An administrator reviews the submission before lending, borrowing or guaranteeing is enabled.
3. **Deposit into the pool.** Verified members add funds and earn a share of loan interest. Available funds can be withdrawn; funds backing a loan or pledge remain locked until released.
4. **Choose a loan.** Select an amount, term and backing from the products below. The engine checks eligibility, credit limits, liquidity and backing. Guarantor pledges must be accepted and any required XLM lock confirmed on Stellar before disbursement.
5. **Repay.** Follow the diminishing-balance repayment schedule. Each payment reduces the outstanding principal, and its interest is split and recorded in whole centavos.
6. **Close and build credit.** Full repayment closes the loan and releases deposit and guarantor backing; XLM returns through the vault's confirmed release process. The score increase waits until the scheduled term has ended, including when a loan is repaid early.

If a loan defaults, recovery follows this order:

```text
Borrower's deposit → Borrower's XLM → Guarantor pledges → Recovery fund → Lending reserve
```

Guarantor claims cover the remaining debt in proportion to each pledge, capped at the pledged amount. Default lowers the borrower's score; a guarantor's score falls when their pledge is actually claimed.

## Loan products

| Product | What backs it |
| --- | --- |
| Deposit-backed | The borrower's own savings, locked behind the loan. |
| XLM collateral | XLM held by the Soroban collateral vault, with a 120% collateral ratio for the sprint and price checks at lock and seizure. |
| Guarantor-backed | The borrower covers at least 50% using their deposit, XLM, or both. Up to two verified guarantors with no unsettled default pledge deposits for the remainder. |

Self-guarantees and duplicate guarantor invitations are refused. Each borrower may have only one open loan, and an unsettled default blocks a new application. Loan rates and limits are fixed at issuance.

## What is built against the SOW

Email verification, KYC, wallet connection and deposit-backed lending were the working baseline before the sprint. The four SOW deliverables extend that flow:

| Deliverable | Implemented behavior | Implementation |
| --- | --- | --- |
| **1. XLM custody** | The vault holds XLM and requires a recorded repayment before release or a recorded default before seizure. Lock and seizure check prices against a SEP-40 feed. The engine verifies transactions, stores hashes and provides database-to-chain reconciliation. | [Vault and runbook](lr_contracts/collateral_vault/README.md), [confirmation](lr_engine/src/api/lending/admin/actions.rs), [reconciliation](lr_engine/src/api/lending/drift.rs) |
| **2. Guarantor product and 50% rule** | Borrower coverage checks, up to two guarantors, invite/accept/decline, locked pledges, release on repayment and proportional claims on default. Eligibility checks run on the server. | [Loan application](lr_engine/src/api/lending/apply.rs), [guarantor lifecycle](lr_engine/src/api/lending/guarantors.rs), [recovery](lr_engine/src/api/lending/recovery.rs) |
| **3. Exact interest split** | Each repayment records the split and recipient allocations using whole-centavo arithmetic, deterministic rounding and balance tests. Guarantor rewards depend on their own score tier. | [Allocation rules and tests](lr_engine/src/api/lending/domain.rs), [repayment](lr_engine/src/api/lending/repay.rs) |
| **4. Term-end credit scoring** | Scores start at 50, with increases capped at 150. A fully repaid loan becomes eligible for an increase only after term end; individual installments, deposits and account activity do not earn increases. Default and guarantor claims carry penalties, with score changes linked to loan history. | [Score processing](lr_engine/src/api/lending/score.rs), [credit history](lr_engine/src/api/lending/credit_history.rs) |

### Interest split at a glance

| Share of interest | Recipient |
| --- | --- |
| 10% | Platform |
| 20% | Lending reserve |
| 40% | Depositors, in proportion to their pool balances |
| 30% | Risk band: guarantor rewards first, then the recovery fund |

A guarantor's tier sets a reward of 10–25% of interest, weighted by their share of the total pledge and paid entirely from the risk band. With no guarantor, the whole risk band goes to the recovery fund.

For example, **₱20.00 of interest** with one entry-tier guarantor pays ₱2.00 to the platform, ₱4.00 to the reserve, ₱8.00 to depositors, ₱2.00 to the guarantor and ₱4.00 to the recovery fund. The parts always sum to ₱20.00.

## How to review the work

The **public Records page** (`/records`, with individual loans at `/records/:loanId`) is the SOW's proof page. It works without an account:

1. Choose a loan and inspect its product, backing, terms and status.
2. Review its repayments, interest splits, score changes and any recovery steps.
3. Follow collateral transaction links to Stellar Expert to inspect the Testnet movements.

The page identifies loans by reference and guarantors by position. Personal details and KYC documents remain private and off-chain. Lending decisions, repayments and guarantor pledges are enforced by the engine; the vault enforces XLM custody on-chain.

Saved evidence:

- [Collateral reconciliation report](lr_engine/evidence/reconciliation-20260926-100956.txt): 21 positions and 28 recorded transactions checked, with zero drift reported for the stated scope. The report records its event window and notes that treasury destination checks were not configured.
- [Interest split test report](lr_engine/evidence/interest-split-20260919-104335.txt): 100,023 calculated splits checked, including the SOW worked example and ten simulated repayments across two guarantor tiers. Recorded repayments can be inspected on Records.

## Code and scope

| Directory | Purpose |
| --- | --- |
| [`lr_frontend/`](lr_frontend/) | React, TypeScript and Vite app, including public Records. |
| [`lr_engine/`](lr_engine/) | Rust/Axum lending engine, PostgreSQL ledger and server-side rules. |
| [`lr_contracts/collateral_vault/`](lr_contracts/collateral_vault/) | Soroban XLM custody contract, tests and build instructions. |
| [`lr_contracts/`](lr_contracts/) | Vault and standalone registry contract code. |

The sprint covers Testnet custody and engine-enforced lending. Registry integration, Maya settlement, mainnet and real-peso lending belong to a later phase. Collateral is valued at issuance and seizure; continuous revaluation and margin calls are outside this scope.

See [ARCHITECTURE.md](ARCHITECTURE.md) for system details and the [vault README](lr_contracts/collateral_vault/README.md) for contract tests, build instructions and the custody runbook. Source code is licensed under [MIT](LICENSE).
