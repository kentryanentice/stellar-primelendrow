# Proposal review and implications for PrimeLendRow Sprint 2

Drafting review • 22 October 2026 • No submission or funding decision is implied.

## Coverage

Reviewed the populated entries in the [shared proposal spreadsheet](https://docs.google.com/spreadsheets/d/1OSyq0keWfYYES-nM22DnQAEGtKwlA4LfZu3D57RN6tU/edit?gid=0#gid=0), including both linked documents for each of Velo, Paiflow, Bullet, Lantern and Orizon Agents. All ten documents were accessible through their public exports. This is a review of the proposed scope and measurement, not an independent verification of their reported users, transactions or completed work.

Salapi, GigPay and PrimeLendRow have named rows but no linked proposals or substantive proposal fields in the supplied tab. There is no proposal content to evaluate for those entries. The blank separator row is excluded.

## Findings by proposal pair

### Velo

The [SOW](https://docs.google.com/document/d/1lCbeLcFGciv44XyC6tYK0EhnYNeDIicjBA06ZoYxGZ8/edit) provides explicit technical acceptance criteria, separates wallet signing from submission, and distinguishes deployment-specific activation from other workflows. Its budget includes customer development within the same request. The [customer development plan](https://docs.google.com/document/d/1dK5YBHUW4g4FFGvFukQAMumk-xtXteC6KguWqsIAvUE/edit) defines unique users, baseline, activation and repeat use consistently with the SOW.

The main improvement is to turn some acquisition targets into testable value hypotheses: why a developer chooses this deployment path and what would disprove its usefulness. The learning and evidence tables still need actual findings and links as work progresses. PrimeLendRow should adopt the shared metric definitions and separate existing members from new participants.

### Paiflow

The [SOW](https://docs.google.com/document/d/1zrWCfi28UgnoBRQI7w3AIWPQ96v4_viE0pNKUtbCqYw/edit) ties safeguards to observable success and refusal cases and bounds its mainnet pilot. The [customer development plan](https://docs.google.com/document/d/1ejuDZJ5OERJmPmI9nAroF_WaX6TEsieFapj_FhUPaU0/edit) separates usability evidence from unvalidated demand and asks whether use continues beyond a hackathon.

Forty onboarded developers and five successful flows measure different things; the plan would be stronger with a participant-level activation measure, a repeat-use window and thresholds for its hypotheses. Mainnet acceptance also depends on readiness checks, so failure of those checks needs an explicit reporting outcome. PrimeLendRow should copy the success/refusal evidence pattern and keep its own sprint on Testnet.

### Bullet

The [SOW](https://docs.google.com/document/d/1thluzhpMj5ClRxtakMJijm-6DTo-OS3HBGfpGMTxl2M/edit) focuses on discovery, inactive users and organizer commitments. The [customer development plan](https://docs.google.com/document/d/14ys5PPoEhISe0yel68rmZnYu6rw5XAXXQkFVGbZidT0/edit) connects hypotheses to thresholds and to decisions if they fail, making it the strongest example here of learning beyond onboarding counts.

There are concrete items to reconcile: SOW Deliverable 1 requires at least five inactive-user interviews, but its success table requires eight; the customer plan retains an “[X of 53]” placeholder; supporting evidence fields remain empty. Recording every session should also have a consent alternative. PrimeLendRow should adopt the decision rules while using one canonical target table and optional recording.

### Lantern

The [SOW](https://docs.google.com/document/d/1IG4a48TDxHR8Fwi87JWrsHF78hQQ_2GG/edit) distinguishes controllable outputs from app-store approval and upstream merges. The [customer development plan](https://docs.google.com/document/d/1ZJzP3tXv48DnsE2i2M0lJogp7UjUCc3X/edit) candidly reports weak return after a campus session and tests whether event reach becomes continuing use.

The technical scope and large event program need clear staffing because the requested budget is described as entirely engineering. Thirty-day return cannot be observed by sprint close for late entrants. The customer plan's later mainnet-readiness wording also needs alignment with the SOW's Week 4 deployment. PrimeLendRow should use a seven-day observation window for participants enrolled early enough and make any later follow-up explicitly post-sprint.

### Orizon Agents

The [SOW](https://docs.google.com/document/d/1tLAZ6GHtRo-yeOwx_UZkth_CdAsra-cZ/edit) measures live agents, completed outside jobs and repeat buyers instead of registrations alone. The [customer development plan](https://docs.google.com/document/d/1IO79AFzOC7sFLwqSeCOXbpz63M6NxeEB/edit) excludes team wallets and distinguishes the reported registration baseline from actual operation and buying activity. Weekly totals and budget breakdowns are explicit.

Several hypotheses are still activity quotas rather than propositions about customer value. A Testnet payment also does not establish willingness to spend real money. Evidence should keep unique people, operators, wallets, agents and jobs separate. PrimeLendRow should adopt that counting discipline: ten wallets controlled by one participant remain one participant and one lending identity.

## PrimeLendRow's existing proposals and code

The earlier local SOW deliberately deferred identity, loan and payment registry integration. This makes identity-based enforcement a distinct follow-on upgrade; it should not request funding again for the completed custody, guarantor, split and scoring work. The earlier customer plan proposed a ten-person expert cohort; its proposed recruitment is not evidence that ten people were actually onboarded.

The current implementation provides useful starting points:

- [Wallet handling](../../lr_engine/src/api/wallets/shared.rs) has a five-wallet cap. Removing that cap is an explicit behavior change.
- The [loan registry](../../lr_contracts/loan_registry/src/lib.rs) keys borrowers by a 16-byte backend identifier, has no KYC-registry lookup, and clears its open-loan index when defaulting. The upgrade must retain an identity-level default restriction until settlement.
- The [payment registry](../../lr_contracts/payment_registry/src/lib.rs) records a backend user identifier and a raw payment-provider reference. The new path needs an opaque receipt key, identity resolution through the loan, and atomic payment/exposure updates.
- The [master's KYC proposal](../master_proposal/Kent_Ryan_Entice_MIT_Identity_Verification_Proposal.docx) describes separate model-training research. Training a replacement OCR/liveness/face-verification stack is not part of this four-week registry upgrade.

## Decisions used in the new drafts

1. Registration, KYC approval, wallet ownership checks, deposit-backed loans, guarantor-backed loans, XLM custody, repayments, interest splits and credit history already exist. The missing work is the on-chain KYC registry and its shared identity reference. Store a stable hash of a private random member reference with an authorised verification status. Wallets and loans refer to that hash; collateral and repayments connect through loan IDs, and existing credit history connects to the same identity. Keep the current ID-number hash private for duplicate-ID checks.
2. There is no fixed wallet-count cap. Each wallet requires proof of control and binds to one identity; linked wallets supply XLM collateral to the existing vault. Adding or unlinking a wallet never resets debt.
3. Loan creation checks identity status and identity-wide obligations inside the contract. XLM-backed loans also check and reserve the collateral address independently of its current identity binding: an unpaid address cannot back another loan under the same or a different identity. A frontend or backend preflight alone is insufficient.
4. Repayments update a loan's balance, collateral-address obligation and history in one contract invocation. Address restrictions clear only when no obligation remains and collateral resolution is confirmed; history stays with the original identity. Fiat receipt facts still depend on the authorized payment reporter.
5. Credit history contains explainable repayment facts. The existing score can consume those facts; the sprint does not claim a validated credit-risk model or a cross-lender credit bureau.
6. The proposed $5,000 request is allocated by work package, with an estimated 224 combined team hours: 172 for technical implementation, validation and documentation, and 52 for preparation, class sessions, admin KYC reviews, access follow-up and evidence. The customer development allocation is $1,100. Confirm team availability before the sprint.
7. Customer development targets 106 onboarded participants: 4 developers beginning in Week 1, 2 MIT/IT teachers in Week 2 and 100 IT college students across four class sessions in Weeks 3 and 4. Plan two classes per week with about 25 new students each; actual class sizes may vary. Count onboarding after registration, login, KYC submission, admin approval and successful system access. Report actual counts, pending cases and any shortfall. Developers return for checks as features become available, and follow-up attendance does not increase the participant total.
8. IT college students are the proposed pilot group because the work involves account, wallet and transaction concepts related to their studies. Recruit through existing capstone teacher contacts. Measure task completion, assistance needed and answers to two clarity questions, then check improvements in later classes. Institutional engagement and research with prospective borrowers, savers and lending staff need a separate proposal. All participation figures are proposed targets.
9. This funding proposal covers Month 2. Use the results to define a separate Month 3 proposal and review longer-term SCF Build requirements with the Chapter Lead. Each funding request has its own scope, targets, budget and approval process.

The privacy wording matters: hashing and separation reduce exposure but do not make linked identity, wallet and credit records anonymous. This follows the distinction in [ICO pseudonymisation guidance](https://ico.org.uk/for-organisations/uk-gdpr-guidance-and-resources/data-sharing/anonymisation/pseudonymisation). The proposed scope excludes direct identifiers from the chain and describes the remaining public linkability.

## Draft package

- [Four-week SOW](PrimeLendRow_Sprint2_SOW.md)
- [Matching customer development plan](PrimeLendRow_Sprint2_Customer_Development_Plan.md)

Planning assumptions: Stellar Testnet, existing KYC decision process, proposed $5,000 combined request, existing team capacity, and a start date to be agreed. Funding eligibility, prior-award totals, recruitment access and publication terms remain to be confirmed before submission. The [official Instawards rules](https://stellar.gitbook.io/scf-handbook/scf-awards/instawards/official-rules) describe Chapter Lead coordination, a bounded sprint and evaluation; these drafts do not imply approval.
