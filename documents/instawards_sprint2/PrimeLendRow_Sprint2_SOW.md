# Instawards Statement of Work (SOW)

Proposed four-week sprint (Month 2)

**Project: PrimeLendRow**

## 1. Project & Team Information

| Field | Details |
| --- | --- |
| Project Name | PrimeLendRow |
| Builder / Team Name | ReverionTech X PrimeLendRow |
| Primary Contact (Name + Email) | Kent Ryan Entice / kent@reveriontech.com |
| Ambassador Chapter | Philippines |
| Ambassador Chapter Lead | Nelson Lumbres |
| Date Submitted | |
| Suggested Sprint Start Date | |

## Executive Summary

PrimeLendRow already supports registration, login, KYC approval, wallet ownership checks, deposit-backed loans, guarantor-backed loans, XLM collateral, repayments, interest splits and credit history. This four-week proposal adds a KYC registry on Stellar that stores a stable member identity hash and verification status, then connects the existing wallet, loan, collateral, repayment and credit-history records to that identity. The work also removes the five-wallet cap and adds shared identity checks to the contracts.

The proposal includes a Stellar Testnet demonstration and a pilot targeting 106 onboarded participants: 4 developers, 2 MIT/IT teachers and 100 IT college students. The evidence covers technical check results, approved system access and changes made in response to class feedback.

## 2. Instawards Overview & Intent

Give PrimeLendRow's existing lending contracts a consistent way to identify a verified member and check their obligations across wallets. The backend already enforces lending rules; this proposal extends verification and obligation checks into connected Stellar contracts through a shared member identity.

The intended result is a complete Testnet integration that reuses the existing loan products, XLM vault, repayments, interest splits and credit rules. Pair technical checks with participant task results to demonstrate correct record linking and identify improvements to the member experience.

## 3. Problem Addressed & Objective

**Problem addressed.** PrimeLendRow is designed for Filipino savers and borrowers who need clear records of loan backing, outstanding balances and repayment history. These records and KYC approval already exist in the application, but the loan and payment contracts lack a shared verified member identity for checking obligations across wallets. Members also face a five-wallet limit.

**Objective.** Connect existing wallets, loans, XLM collateral, repayments and credit history to one verified member identity on Stellar Testnet, preserving obligations and history when wallets change. Remove the five-wallet limit. Target 106 onboarded pilot participants: 100 IT college students, 4 developers and 2 MIT/IT teachers. Count onboarding after admin KYC approval and successful system access. Measure wallet-task completion, understanding of loan and repayment records, and assistance needed. Use the same core tasks across class sessions to check improvements, and report actual results, unresolved issues and any shortfall.

### 3.1 Already implemented and proposed work

| Component | Already implemented | Work in this proposal |
| --- | --- | --- |
| Onboarding and KYC | Registration, login, private KYC submission, admin review and approval, then system access. | Add the KYC registry to store a stable member identity hash and verification status. Connect existing admin decisions to that record. |
| Wallets | Wallet ownership checks and account linking, with a five-wallet limit. | Link wallets to the same identity hash, remove the limit and preserve obligations when links change. |
| Deposit-backed loans | Loans backed by the borrower's locked deposits, with eligibility and balance checks in the backend. | Apply the shared identity and loan checks to the existing product, retaining deposit-backing records. |
| Guarantor-backed loans | Guarantor invitations, accepted pledges, locked backing, release on repayment and claims on default. | Link the loan to the borrower's shared identity and preserve the existing guarantor rules and pledge records. |
| XLM-collateral loans | Backend lending checks and vault locking, release and seizure. Collateral records use the loan ID and wallet address. | Connect each loan to the identity hash and its existing vault record. Add contract checks for verification, member debt and unpaid collateral addresses. |
| Repayments | Payment validation, balance updates and repayment records in the backend. | Integrate payment registry records with the shared identity through the loan, with consistent balances and duplicate-payment checks. |
| Interest splits | Recorded allocations of repayment interest to the platform, lending reserve, depositors, guarantors and recovery fund. | Reuse the existing split calculations and preserve allocation records when repayments are linked to the shared identity. |
| Credit history | Credit-history views, score-change records and term-end scoring rules. | Connect the existing history and scoring records to the shared identity's loan and repayment history. |

**How the hash connects the records.** The new KYC registry stores the member identity hash and verification status. Wallet and loan records refer to that hash. XLM collateral records and repayments connect through their loan IDs, and the application brings the credit history together under the same identity.

Use a hash of a private random member reference for the on-chain identity. The existing ID-number hash remains private and serves duplicate-ID checks. KYC documents stay off-chain.

### 3.2 Contract integration example

A member takes an XLM-backed loan using Wallet A. While it is unpaid, they cannot borrow again using Wallet B. Wallet A also cannot back another loan under a different member. Partial payment or removing the wallet link does not clear the debt.

### 3.3 Why Stellar

PrimeLendRow already uses Stellar wallets and a Soroban vault for XLM. Connecting the registries on the same network lets contracts check the same verification and loan records. Publish the contract changes and a short integration guide so other builders can review and reuse the approach.

### 3.4 Proposed pilot

Begin with 4 developers in Week 1 and 2 MIT/IT teachers in Week 2. Developers return for checks as features become available. Recruit 100 IT college students through existing capstone teacher contacts for four class sessions in Weeks 3–4: two per week, with about 25 new students per class. Class sizes may vary. Use feedback from each class to improve the next.

### 3.5 Key risks and controls

| Risk | Mitigation |
| --- | --- |
| Exposing personal information | Hash a private random identity reference. Keep names, documents and biometrics off-chain; obtain permission for public wallet links before migration. |
| Unauthorised verification or wallet linking | Limit KYC updates to authorised reviewers and require proof of wallet ownership. |
| Bypassing unpaid debt | Check obligations against both the member and the XLM collateral address, including after default, partial payment or unlinking. |
| Lost obligations or duplicate requests | Migrate open loans and unresolved defaults before enabling borrowing. Block incomplete records and prevent duplicate loan or repayment effects. |

## 4. Scope of Work (30-Day Deliverables)

The proposal covers changes to the existing application, registries and collateral vault, with validation on Stellar Testnet using sample funds.

### 4.1 In-Scope Deliverables

| Deliverable | Proposed work | Why it matters |
| --- | --- | --- |
| 1. KYC registry | Create the registry for each member's stable identity hash and verification status. Connect existing admin approval, plus revocation, expiry and re-verification. Private KYC records stay off-chain. | Contracts share the same verification status. |
| 2. Wallet linking | Reuse ownership checks, remove the five-wallet cap and link each wallet to the identity hash. Refuse reassignment of an unpaid address; preserve obligations and history after unlinking. | Members can change wallets while retaining their history. |
| 3. Loan registry | Extend the existing loan contract with the KYC identity reference and its link to the vault record. Enforce verification and borrowing restrictions on-chain. Preserve balances and restrictions during migration. | Borrowing checks follow the member and collateral address. |
| 4. Repayments and credit history | Integrate the existing payment registry with each loan's identity reference. Record payments once and connect loan, settlement and credit history. Allow revoked members to repay; retain existing interest splits and term-end scoring, and clear address restrictions only after debt and collateral resolution. | Repayment history stays consistent across wallets. |

**Customer development.** Count each participant once after KYC approval and successful system access. Record task results, feedback and any onboarding shortfall. The requested budget includes 52 team hours for this work.

**Out of scope.** Mainnet, real-money lending, new payment rails, a new KYC provider, a new credit-scoring model, cross-lender integration, institutional pilots and a third-party audit.

### 4.2 Deliverable-Aligned Budget Request

Request **$5,000** for an estimated **224 combined team hours**, allocated by work package. Technical estimates include integration, testing and documentation.

| Work | Team hours | Amount (USD) |
| --- | --- | --- |
| KYC registry | 44 | $1,000 |
| Wallet linking | 36 | $800 |
| Loan registry | 52 | $1,200 |
| Repayments and credit history | 40 | $900 |
| Customer development and evidence | 52 | $1,100 |
| **Total** | **224** | **$5,000** |

Customer development covers preparation (6 hours), six sessions with two team members (20), admin KYC reviews and access follow-up (16), and feedback and evidence (10). Developer follow-up checks are included in technical validation. Confirm team availability and class arrangements before the sprint.

## 5. 30-Day Execution Plan & Timeline

Start Week 1 on the date agreed with the Chapter Lead. The estimates total 172 technical hours and 52 customer development hours. The schedule allows four weeks within 30 days, with weekly allocations adjusted to class schedules and review needs.

| Week | Planned work | Expected output |
| --- | --- | --- |
| Week 1 — 54 hours | Build and test the KYC registry, connect approvals, migrate identity records and begin onboarding 4 developers. | Sample identity records, migration summary and developer access results. |
| Week 2 — 52 hours | Complete wallet linking, begin loan integration, run developer wallet checks and onboard 2 teachers. | At least 6 wallets under one identity and feedback on instructions. |
| Week 3 — 62 hours | Complete loan checks and obligation migration; begin repayment integration. Run two student classes, admin reviews and developer loan checks. | Successful and refused loan examples, onboarding counts and changes for the next classes. |
| Week 4 — 56 hours | Complete repayment and full-flow checks. Run two more student classes, follow up on access and prepare the final evidence. | At least 5 loans and 10 repayments validated; pilot results and remaining issues. |

## 6. Evidence of Completion (Required)

Provide one evidence index with a result and reference for each check below. Mark failed or pending checks clearly. Use sample identities and funds on Testnet; label controlled test dates used for late-payment and term-end scenarios.

### 6.1 Planned Evidence to Be Submitted

| Deliverable | Evidence type | What it demonstrates |
| --- | --- | --- |
| 1. KYC registry | Contract ID, transactions and check results | At least 5 sample identity hashes and verification statuses recorded in the registry. Approval, revocation and expiry work; re-verification keeps the same identity; unauthorised updates are refused. Names, ID details and biometrics remain off-chain. |
| 2. Wallet linking | Screenshots, recording and check results | At least 6 wallets under one identity. Ownership is checked when adding or removing links. An unpaid address cannot be reassigned, and the same history is available through two wallets. |
| 3. Loan registry | Transactions, migration summary and check results | Trace a loan and its vault collateral record to the same identity hash. A valid loan succeeds. Unverified, revoked or expired members, second open loans and unresolved defaults are refused. An unpaid address stays blocked under either identity after partial payment or unlinking. Migration preserves balances and restrictions; incomplete migration blocks borrowing. Duplicate requests create no extra loan or reservation. |
| 4. Repayments and credit history | Payment links, history and check results | At least 5 loans and 10 repayments cover completion, late payment, default and settlement. Trace repayments and credit history to the loan's identity. Revoked members can repay. Duplicate payments have no extra effect. Eligible score increases occur once, after full repayment and term end. Balances reconcile; address restrictions clear only when no debt remains and collateral release or seizure is confirmed. |
| Customer development | Counts, access evidence and feedback | Attendance, KYC submissions, approvals and successful access by class. Task results, clarity answers, assistance needed, changes between classes and any shortfall. |

Include code links and the integration guide. Handle participant evidence as described in the customer plan.

### 6.2 Evidence Verification Checklist (For Ambassador Use)

| Deliverable | Present | Partial | Missing | Comments |
| --- | --- | --- | --- | --- |
| 1. KYC registry | ☐ | ☐ | ☐ | |
| 2. Wallet linking | ☐ | ☐ | ☐ | |
| 3. Loan registry | ☐ | ☐ | ☐ | |
| 4. Repayments and credit history | ☐ | ☐ | ☐ | |
| Customer development | ☐ | ☐ | ☐ | |

### 6.3 Success Metrics

| Metric | Target |
| --- | --- |
| Technical sample | At least 5 identities, 6 wallets under one identity, 5 loans and 10 repayments |
| Required feature checks | All checks in Section 6.1 pass, with evidence; report unresolved failures |
| Duplicate effects or unexplained balance differences | 0 in the recorded checks |
| Personal KYC data on-chain | 0 names, ID details or biometric records |
| Pilot participants onboarded | Target 106: 4 developers, 2 MIT/IT teachers and 100 IT college students; report actual approved access and shortfalls |
| Pilot learning | Task results, feedback and changes checked in later classes |
| Reviewer evidence package | Complete by Week 4 |

## 7. Next-Step Alignment

Review the sprint results with the Chapter Lead to define a separate Month 3 proposal, including its scope, targets and budget. Review SCF Build requirements as a longer-term funding option.

Research with borrowers, savers and lending staff, institutional pilots, mainnet and real-money lending require separate scope and funding.

Next-step options:

☐ SCF Build application

☐ Continue development independently

☐ Follow-on Instaward proposal for Month 3

☐ Seek other ecosystem support

☐ Other

## 8. Instawards Constraints Acknowledgement

By submitting this SOW, the Builder acknowledges:

☑ Proposed duration: four weeks within 30 days.

☑ Instawards support execution, not open-ended exploration.

☑ A project may receive no more than two follow-on Instawards.

☑ This proposal requests $5,000, subject to review and approval.

☑ Total Instawards funding may not exceed $15,000.

Funding and any follow-on award are subject to the [official Instawards rules](https://stellar.gitbook.io/scf-handbook/scf-awards/instawards/official-rules) and approval. Confirm prior award totals with the Chapter Lead before submission.

## 9. Submission Confirmation

Once finalized, complete the submission date, agree the proposed start date with the Chapter Lead, and submit the SOW with their support for review and approval.

## Appendix A — What Already Exists

| Component | Status |
| --- | --- |
| Registration, login, private KYC and admin review | Implemented; approval already enables system access. KYC registry creation and synchronisation are proposed work. |
| Wallet authentication and management | Implemented with ownership checks and a five-wallet cap. Linking wallets to the registry identity is proposed work. |
| PayPal and Stripe deposits and withdrawals | PayPal working; Stripe integrations implemented, with sandbox testing planned before live activation. Both support sandbox and live configurations. |
| Deposit-backed loans, guarantor-backed loans and XLM custody | Implemented in the engine and vault; the shared KYC identity connection is proposed work. |
| Interest splits, term-end credit scoring and credit history | Implemented in the backend, including recorded recipient allocations; this proposal connects the records to the registry identity. |
| Loan and payment registry code | Standalone contracts exist using member and loan IDs. KYC identity integration and application wiring remain to be completed. |

