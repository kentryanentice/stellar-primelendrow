# PrimeLendRow — Workflow

This document walks through the PrimeLendRow user journey end to end: registration,
KYC, wallet connection, lending, borrowing, guarantorship, repayment, default handling,
and on-chain anchoring. For how the system is built to support this journey, see
[ARCHITECTURE.md](ARCHITECTURE.md).

---

## 1. Registration

A user creates an account using an email address, username, and password. An OTP is sent through Resend Email and must be verified before account creation is completed.

**Initial State**
- Role: `Pending`
- Credit Score: `50`
- Wallet: `Not Connected`
- KYC Status: `Not Submitted`

---

## 2. Authentication

The user logs in using their email or username and password.

**Available Features**
- View public lending pool statistics
- Manage account settings
- Submit KYC verification

**Restricted Features**
- Lending
- Borrowing
- Guarantor participation
- Wallet-dependent operations

---

## 3. KYC Verification

The KYC wizard is five steps, in order — a wallet must be connected before the
user can review and submit.

### Step 1: Government ID Upload

Supported IDs:

- PhilSys ID
- Driver's License
- Passport
- Postal ID

### Step 2: ID Scan

The system performs OCR on the uploaded ID and extracts:

- Full Name
- Date of Birth
- ID Number
- ID Type

The user reviews and can correct any field before continuing.

### Step 3: Liveness Verification & Face Matching

The user performs a liveness challenge (blink detection on the live camera
feed, with anti-spoof checks). The system automatically captures a facial
image once the challenge passes, then compares it against the ID portrait.

Extracted values:

- Similarity Percentage
- Match Result
- Confidence Score

### Step 4: Wallet Connection

The user connects a Stellar-compatible wallet — this step is part of the KYC
flow itself and is required before the user can continue to review and
submit. This wallet becomes the account's **KYC-verified wallet** (see
[Section 4](#4-wallet-management)).

### Step 5: Review & Submission

The user reviews the extracted information, the face-match result, and the
connected wallet address, then submits the verification request.

**Status Changes**

```text
Not Submitted
    ↓
Submitted
    ↓
Under Review
```

### Step 6: Administrative Review

An administrator reviews the submitted KYC information, uploaded identification document, OCR results, liveness verification, and face matching results.

**Approval Flow**

```text
Under Review
    ↓
Approved
    ↓
Role = User
```

**Rejection Flow**

```text
Under Review
    ↓
Rejected
    ↓
Re-Verification Required
```

Rejected users may correct their information and submit a new verification request.

---

## 4. Wallet Management

The wallet connected during KYC (Section 3, Step 4) is saved as the account's
**KYC-verified wallet** and is already active once the account is approved —
there is no separate first-connection step to complete afterward.

From Settings, a user may connect **additional** Stellar-compatible wallets:

- Freighter
- Lobstr
- Albedo
- Other Stellar-compatible wallets

### Step 1: Connect an Additional Wallet

Each new wallet is verified with a signature challenge before it's linked —
the wallet must prove it holds the private key behind the address, not just
supply the address:

```text
Not Connected
      ↓
Challenge Issued
      ↓
Signature Verified
      ↓
Connected
```

A user may optionally label a wallet (e.g. "Savings", "Mobile") when
connecting it. Up to **5 active wallets** per user are allowed; the limit
must be freed up (by disconnecting one) before another can be added.

### Step 2: Disconnect a Wallet

Any wallet — the KYC-verified one included — can be disconnected:

```text
Connected
    ↓
Disconnected
```

Disconnected wallets are kept, not deleted, and remain visible as history.

Stored wallet information:

- Wallet Address
- Label (optional)
- Source (KYC-verified or user-added)
- Status (Active or Disconnected)
- Connected / Disconnected Timestamps

---

## 5. Lending

### Step 1: Deposit Funds

Users deposit funds into the lending pool.

**Status Changes**

```text
No Deposit
     ↓
Deposited
     ↓
Available
```

Deposited funds may be:

- Withdrawn
- Used as borrowing collateral
- Used as guarantor collateral

### Step 2: Fund Reservation

When funds are assigned to an active loan or guarantor agreement:

```text
Available
    ↓
Reserved
    ↓
Locked
```

Locked funds cannot be:

- Withdrawn
- Borrowed against
- Used to guarantee another loan

### Step 3: Withdrawal

If funds are not reserved or locked:

```text
Available
    ↓
Withdrawal Request
    ↓
Withdrawn
```

---

## 6. Borrowing

### Step 1: Loan Request

The user submits:

- Loan Amount
- Loan Term
- Collateral Source
- Guarantor Selection (Optional)

### Step 2: Eligibility Validation

The system validates:

- User Role
- KYC Approval
- Credit Score
- Loan Limit
- Available Liquidity
- Collateral Coverage

**Validation Result**

```text
Validation Failed
        ↓
Loan Rejected
```

or

```text
Validation Passed
        ↓
Loan Processing
```

### Step 3: Collateral Assignment

Collateral may come from:

- User Deposit
- User XLM
- Guarantor Deposit
- Guarantor XLM

**Collateral Status**

```text
Available
    ↓
Assigned
    ↓
Locked
```

---

## 7. Guarantor Assignment

### Step 1: Guarantor Request

The borrower selects up to two guarantors.

The guarantor receives a loan support request.

**Status Changes**

```text
Pending
    ↓
Accepted
```

or

```text
Pending
    ↓
Declined
```

### Step 2: Guarantor Lock

Upon acceptance:

```text
Guarantor Collateral
          ↓
Locked
```

The collateral remains locked until:

```text
Loan Closed
        ↓
Collateral Released
```

or

```text
Loan Defaulted
        ↓
Collateral Liquidated
```

---

## 8. Loan Creation

### Step 1: Loan Approval

After all validations and collateral requirements are satisfied:

```text
Pending
    ↓
Approved
```

### Step 2: Fund Disbursement

The approved loan amount is released.

**Status Changes**

```text
Approved
    ↓
Active
```

Generated records include:

- Loan ID
- Borrower ID
- Guarantor IDs
- Loan Amount
- Interest Rate
- Loan Term
- Repayment Schedule

---

## 9. Repayment

### Step 1: Monthly Installment Generation

The system calculates monthly payments using a diminishing balance model.

**Example**

```text
Loan Amount: ₱1,000
Term: 3 Months
Interest: 2% Monthly
```

Month 1

```text
Principal: ₱333.33
Interest: ₱20.00
```

Month 2

```text
Principal: ₱333.33
Interest: ₱13.33
```

Month 3

```text
Principal: ₱333.33
Interest: ₱6.67
```

### Step 2: Payment Submission

The borrower submits a payment.

**Status Changes**

```text
Due
  ↓
Paid
```

The remaining principal balance is updated after every successful payment.

---

## 10. Loan Completion

### Step 1: Final Payment

The final installment is received.

**Status Changes**

```text
Active
   ↓
Paid
   ↓
Closed
```

### Step 2: Collateral Release

Borrower collateral is unlocked.

```text
Locked
   ↓
Released
```

Guarantor collateral is unlocked.

```text
Locked
   ↓
Released
```

### Step 3: Credit Score Update

Only completed loans are eligible for score increases.

```text
Loan Closed
      ↓
Score Evaluation
      ↓
Credit Updated
```

---

## 11. Default Processing

If repayment obligations are not fulfilled:

**Status Changes**

```text
Active
   ↓
Overdue
   ↓
Defaulted
```

Collateral recovery sequence:

```text
Borrower Deposit
        ↓
Borrower XLM
        ↓
Guarantor Deposit
        ↓
Guarantor XLM
```

After liquidation:

```text
Defaulted
      ↓
Closed
```

Credit penalties are applied.

---

## 12. Stellar Blockchain Anchoring

### Step 1: Financial Event Creation

A blockchain event is generated for:

- Deposit
- Withdrawal
- Loan Creation
- Repayment
- Loan Closure
- Collateral Lock
- Collateral Release
- Default Processing

### Step 2: Hash Generation

```text
Financial Event
       ↓
Canonical Payload
       ↓
SHA256 Hash
```

### Step 3: Blockchain Submission

```text
SHA256 Hash
       ↓
Stellar Transaction
       ↓
Confirmed
```

Only cryptographic proofs are anchored on-chain.

No personal information, KYC documents, ID numbers, facial images, or sensitive user information are stored on the blockchain.
