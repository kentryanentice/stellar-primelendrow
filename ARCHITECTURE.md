# PrimeLendRow — Cash Pool Lending, Taught From Zero

This document is the architecture of PrimeLendRow, written as a lesson. Read it top
to bottom once and you should be able to explain the whole system to
someone else — that's the test. Every lesson ends with **Remember** lines;
if you keep only those, you still pass.

The one style rule behind every choice here: **if a beginner can't run it,
debug it, and audit it alone, it's too complicated.** Boring on purpose.

---

## Lesson 1 — What PrimeLendRow is

Think of a *paluwagan* with rules written in code.

Everyone puts money into one shared pot. Members borrow from the pot.
Friends can vouch so a member can borrow more. When borrowers pay interest,
that money flows back to the people who saved.

The real cash moves through GCash, Paypal, or bank — outside the app. The
app is two things that never lie:

- **the notebook** — every event that ever happened, written forever,
  never edited, never erased;
- **the books** — accountant-style records that must always balance, to
  the centavo.

> **Remember:** cash moves outside; truth lives inside. The notebook tells
> the story, the books count the money, and they are written together.

---

## Lesson 2 — The ten decisions (locked)

These were argued, decided, and written down so nobody quietly re-decides
them later.

**D1. Collateral freezes — and everyone is told.**
Savings backing a loan (yours, or a pledge for a friend) stay frozen until
that loan closes — even if their 6-month lock has already ended. The app
shows you the new dates *before* you sign, and your "I understand" is
recorded as an event. This is why recovery (Lesson 8) is always funded.

**D2. Two loan products, two honest prices.**

| Product | Backed by | Price (illustrative) | Why |
|---|---|---|---|
| **Advance** | your own savings only | flat 2%/month | the pot risks almost nothing — price it that way |
| **Community Loan** | savings + friends' pledges | 5–8%/month by score | trust is what the pledges are betting on |

**D3. Payouts are cash-only.**
Savers split the interest that was **actually received** this month.
Interest that is owed but unpaid is *tracked* in the books — but tracked
is not distributable. No one is ever paid from imaginary money.

**D4. Deposits are never refused — money waits in the shallow end.**
Every deposit is accepted into one of two pools:

```
IDLE POOL     withdraw anytime · earns nothing · never lent, never at risk
ACTIVE POOL   the 6-month lock AND the earnings both start at activation
```

Activation is first-come-first-served, automatic when lending demand
exists. The deposit screen says it honestly: *"The pot is currently fully
funded. Your money activates in queue order; lock and earnings start then.
Until then, withdraw anytime."* The app always shows **utilization**
("the pot is 43% lent out") so nobody wonders why yield is what it is.

Two honest consequences: idle money earning nothing is the *price* of never
refusing anyone (shown, not hidden), and holding idle cash is custody of
public funds — which adds weight to D7. Fallback if lawyers prefer it:
"pledge now, transfer only at activation" — same queue, no custody.

**D5. Two numbers per member, two different jobs.**
One trust score has a long-term disease: after five good years everyone
converges to the top and the number stops meaning anything. So, split it:

```
SCORE          "Do you keep your word?"    10–150 · moves ±2/±20 on
               behavior · sets your PRICE (8% → 5%/month) · can always fall
TRACK RECORD   "How much have you proven?" pesos repaid on time, lifetime ·
               only grows · sets your CEILING
```

The ceiling ladder (illustrative, tunable):

| Repaid on time, lifetime | Most you may borrow |
|---|---|
| ₱0 (new member) | ₱5,000 |
| ₱10,000 | ₱10,000 |
| ₱30,000 | ₱20,000 |
| ₱80,000 | ₱40,000 |
| ₱200,000 | ₱65,000 |
| ₱450,000 | ₱90,000 |

Plus three brakes: your next loan ≤ **2× your largest fully-repaid loan**;
a cooldown before first max-ceiling borrowing; after a default your ceiling
floors at the entry rung for 12 months, no matter your numbers. The only
road to borrowing big is *repaying* big — an exit scam has no shortcut.

**D6. The reserve — airbag first, backstop second, size computed.**
The reserve's first job is operational disasters (rail failures, fraud,
admin error). Its second job, stated plainly: it absorbs whatever residue
survives after a defaulter's collateral and pledges are used up. Its target
is *derived monthly*, never hand-picked:

```
target = max( 2 × the largest amount one admin may approve alone,
              3 months of operating costs,
              the worst operational loss seen in rehearsals (Lesson 12),
              5% of active loans — an absolute floor )
```

And the interest split follows reserve health — with a non-negotiable zero:

| Reserve vs. target | Savers | Platform | Reserve | New loans? |
|---|---|---|---|---|
| ≥ 100% | 70 | 10 | 20 | yes |
| below 100% | 60 | **0** | 40 | fully-backed only |
| below 50% | 30 | **0** | 70 | paused |
| empty | 0 | **0** | 100 | paused |

**When savers are at risk, the platform earns nothing.**

**D7. No real pesos before the legal wrapper.**
The moment an admin matches a *real* bank statement, the system is in real
operation — "manual" is not a halfway state:

```
SimulationRail   fake pesos, real rules      → allowed BEFORE the wrapper
ManualRail       real pesos, human matching  → requires the wrapper
ProviderRail     real pesos, API-confirmed   → requires the wrapper
```

Which wrapper (cooperative RA 9520 / NSSLA RA 8367 / licensed lending
company) is a lawyer's question, outside this document — but it gates
*both* real rails.

**D8. Rules are data; changing a rule is an event.**
Every parameter (rates, ladders, splits) lives in versioned sets. Changing
one writes an event. Live loans keep the parameters they were born with.

**D9. The money accounts for itself.**
Events tell the story ("loan disbursed"). They don't answer the
accountant's question: *which balances changed?* So every event also writes
a balanced accounting entry — both sides always equal. At every second:

```
assets  =  what members put in  +  reserve  +  platform earnings
```

Banks don't trust logs; they trust books that balance. PLR has both,
written by the same transaction. (Worked examples in Lesson 6.)

**D10. The queue is first-come-first-served — on purpose.**
When cash is scarce, a score-140 member can wait behind a score-50 member
who asked first. That is deliberately "inefficient": a paluwagan serves the
line, not the VIP. It's written down precisely so nobody quietly
"optimizes" it later and changes what the platform *is*. (Everyone in the
queue already passed every safety check — the queue decides *when*, never
*whether*.)

> **Remember:** frozen collateral is announced, two loan products, cash-only
> payouts, deposits never refused, score = price / track record = ceiling,
> reserve target is computed, no real money before the wrapper, rules are
> versioned data, books always balance, the queue is fair not clever.

---

## Lesson 3 — The shape of the machine

```
lr_frontend ──HTTP──▶ lr_engine ───▶ PostgreSQL
 (a window)           (the brain)    (the notebook + the books)
                         │
                         ├───▶ lr-mailer  (sends emails)
                         └───▶ lr-cdn     (stores ID photos, privately)
```

One Rust program, one repository, one database. The core promise — money,
trust, and accounts change together or not at all — is trivially true
inside one database transaction, and genuinely hard across many services.

The frontend is a **window, not a calculator**. It never computes money; it
only shows what the brain decided.

One sentence for the future: this stays a single program until *measured*
operational pain justifies splitting it — not before, and not by fashion.

> **Remember:** one brain, one notebook. The window never does math.

---

## Lesson 4 — The one rule everything obeys

Every action — deposit, loan, payment, vouch, admin click — walks the same
four steps inside **one** all-or-nothing transaction:

```
        a command arrives
              │
   ┌──────────▼──────────────────────────┐
   │ 1. VALIDATE — pure rule functions   │
   │ 2. APPEND   — events → notebook     │
   │ 3. POST     — balanced entries →    │
   │               the books             │
   │ 4. PROJECT  — update the summaries  │
   └──────────┬──────────────────────────┘
              ▼
      reply / queue / refuse
```

If any step fails, all of it rolls back. There is no moment — not even a
millisecond — where the story, the books, and the balances disagree.

> **Remember:** validate → append → post → project. All or nothing.

---

## Lesson 5 — Every peso wears exactly one badge

Every centavo in the pot has exactly one of five badges:

```
IDLE        resting in the shallow end — withdrawable, unlent, no yield
LOCKED      activated; inside its 6-month lock; earning
AVAILABLE   lock finished, backing nothing — may be withdrawn
COLLATERAL  backing YOUR loan    — frozen until that loan closes (D1)
PLEDGED     backing a FRIEND's   — frozen until that loan closes (D1)
```

The only allowed moves (code refuses everything else):

```
deposit confirmed ───────────────────▶ IDLE
IDLE ──(activated, first-come-first-served)──▶ LOCKED  (clock + yield start)
IDLE ──(withdraw anytime)────────────▶ out of the pot
LOCKED ──(6 months, backing nothing)─▶ AVAILABLE
LOCKED/AVAILABLE ──(you borrow)──────▶ COLLATERAL
LOCKED/AVAILABLE ──(you vouch)───────▶ PLEDGED
COLLATERAL/PLEDGED ──(loan closes)───▶ LOCKED if the clock still runs,
                                       otherwise AVAILABLE
COLLATERAL/PLEDGED ──(day-60 default)▶ seized, in the recovery order
AVAILABLE ──(withdraw)───────────────▶ out of the pot
```

Three facts are re-checked by automated tests on every change to the code:

1. badge totals = member deposits = pot cash + loans outstanding;
2. no centavo ever wears two badges;
3. IDLE money never enters lending math — an idle withdrawal can never
   threaten a loan.

> **Remember:** five badges, one badge per peso, and idle money is
> untouchable by loans.

---

## Lesson 6 — The books, in two examples

Read a posting as *"where it came from | where it went."* Both sides must
total the same, always.

A member's ₱5,000 deposit is confirmed:

```
DepositConfirmed ₱5,000
  Cash                 5,000  |  Member Deposits      5,000
```

A ₱1,080 repayment arrives (₱1,000 principal + ₱80 interest; the reserve is
at target, so the 70/10/20 split applies to the interest):

```
RepaymentReceived ₱1,080
  Cash                 1,080  |  Loans Receivable     1,000
                              |  Saver Payout Payable    56
                              |  Reserve Fund            16
                              |  Platform Earnings        8
  ────────────────────────────┼─────────────────────────────
  total                1,080  |  total                1,080
```

The entire chart of accounts stays this small: **Cash, Loans Receivable |
Member Deposits, Saver Payout Payable | Reserve Fund, Platform Earnings** —
plus one memo account for interest owed-but-unpaid, which is *tracked* but
never *distributed* (D3). One posting rule per event type; a test asserts
the books balance after every event in every simulated history.

> **Remember:** every event writes a balanced entry. ₱80 of interest splits
> 56 / 16 / 8. Owed-but-unpaid is tracked, never paid out.

---

## Lesson 7 — The member's four numbers

Every money app eventually converges on the same four questions. PLR puts
them on the home screen from day one:

```
AVAILABLE NOW     what you could withdraw today (IDLE + AVAILABLE)
FROZEN            what's working or backing — with its dates
                  (LOCKED till Aug 12 · COLLATERAL till loan closes …)
AT RISK           what you could LOSE if things go wrong
                  = COLLATERAL + PLEDGED
BORROWING POWER   the smallest of your limits, right now
```

One deliberate choice: what you could **lose** (collateral + pledges) and
what you **owe** (your loan) are two different fears — PLR never adds them
into one number. *Never add pesos that mean different things.*

> **Remember:** available, frozen, at risk, borrowing power — and owing is
> not the same as risking.

---

## Lesson 8 — A loan's life, start to finish

**Birth:**

```
apply (Advance or Community — D2)
  │
  ├─ limits     ceiling from track record (D5), 2× rule, one loan only
  ├─ liquidity  whole-life cash check on STRESSED numbers
  │               (assume only 80% of repayments arrive — tunable)
  ├─ reserve    at or above target? (D6)
  ├─ risk       private approve / review / decline check
  ├─ consent    D1 freeze-extension shown in real dates → recorded
  │
  ├─ all pass ──▶ ONE transaction: approved + collateral frozen
  │               (+ pledges frozen) + postings + badges + parameters
  │               pinned (D8) · then the rail pays out
  │
  └─ cash short ──▶ QUEUED — first come, first served (D10),
                    your place visible
```

**Repayment:** real money confirmed by the rail (duplicates physically
blocked — Lesson 9) → one transaction: postings (Lesson 6) → allocation
(overdue interest → current interest → principal) → score +2 and streak
bonus → track record grows by the pesos repaid on time → split by the D6
table. If the loan just closed: collateral and pledges released, +10 score,
voucher bonus (with the 5 → 2 → 1 → 0 repeat decay).

**When payments stop, nothing is sudden:**

| Day | What happens |
|---|---|
| due + 3 | grace ends — now it's late |
| day 7 | written warning |
| day 30 | a person reaches out |
| day 45 | final notice — vouchers are alerted |
| day 60 | default — recovery runs |

**Recovery order (never violated):**

```
1. the borrower's own COLLATERAL
2. the vouchers' PLEDGES (pro-rata by size)
3. the Safety Reserve (D6's backstop role)
   — savers' deposits: never
```

> **Remember:** stressed liquidity check before birth; allocation order
> overdue-interest → interest → principal; day 3/7/30/45/60; recovery
> borrower → vouchers → reserve, never savers.

---

## Lesson 9 — The clock and the database

**The clock** is a set of small jobs. Each one checks the notebook before
acting, so a crash and re-run can never duplicate anything:

```
activation   IDLE → LOCKED promotions, strict first-come-first-served (D4)
dunning      the day 3 / 7 / 30 / 45 / 60 path (Lesson 8)
unlocks      LOCKED → AVAILABLE at maturity (unless frozen — D1)
payouts      monthly · points = amount × time active × 1.5 if pledged ·
             paid strictly from COLLECTED interest (D3)
queue        promotes waiting loans when repayments free up cash (D10)
```

**The database** has three layers:

```
events        the notebook · INSERT only — UPDATE and DELETE are revoked
postings      the books · same protection · a database rule checks
              debits = credits on every single event
projections   summaries (balances, scores, the pot, the queue) —
              disposable; rebuilt anytime by replaying events
```

Two hard-won details, promoted to rules:

- **Payments can't post twice.** Every real-money confirmation carries the
  provider's reference number, and the database refuses a duplicate.
  Providers *will* send the same callback twice; the schema shrugs.
- **Snapshots, later.** Replaying thousands of events is instant; millions
  are not. When that day comes, periodic snapshots (each one an event
  itself) let replay start near the end. Designed for, not built yet.

> **Remember:** jobs are re-runnable; events and books are append-only;
> summaries are disposable; duplicate payments bounce off the schema.

---

## Lesson 10 — The map of the code

`lr_engine` in one glance — each folder has one job:

```
api/         the front door — HTTP in, JSON out, zero business rules
             (routes live under /api/v1 so breaking changes get /api/v2)
domain/      PURE rules — money, badges, score, track, pricing, limits,
             liquidity, reserve, payout math · no database, no clock
ledger/      the notebook + the books — events, accounts, the
             one-transaction writer, projections
workflows/   the stories — deposit, loan, repayment, vouch, recovery,
             payout — gluing domain + ledger together
risk/        suspicion in one place — the private approval check, fraud
             flags, anomaly watch (feeds auto-pause), the dashboard numbers
jobs/        the clock (Lesson 9)
policy/      the rulebook — versioned parameters, D8
infra/       the outside world — database, mailer, files, and the rails
             (Simulation | Manual | Provider — the real two gated by D7)
```

The one dependency rule: `domain/` imports nothing below it. **If it can't
be tested with plain numbers, it's in the wrong folder.**

> **Remember:** rules are pure, stories glue, suspicion lives in one room,
> and the door does no math.

---

## Lesson 11 — Rehearsing disasters

Three levels of tests, run on every change:

```
unit        score math, ladders, pricing, splits, badge moves
property    thousands of random histories, asserting always:
              · badge sums reconcile      · no negative balances
              · one active loan each      · frozen money never moves
              · the books balance after every event
              · idle pool = its cash, exactly
simulation  whole villages, months of life, replayed:
              · 10% default at once — does recovery + reserve hold?
              · 30% withdraw at unlock — is the cash there?
              · 20% pay late — payouts shrink honestly, never fake
              · the biggest voucher defaults — who loses, exactly?
              · a typhoon — defaults + withdrawals + rails down, one region
              · reserve at zero, platform share zero for months —
                what funds operations? the answer must exist in writing
```

If a rule change makes any rehearsal fail, the change is blocked.

> **Remember:** unit, property, simulation — and the typhoon is on the list.

---

## Lesson 12 — Running it for real

**Backups you've rehearsed.** Nightly full backup plus continuous
write-ahead archiving: lose at most ~5 minutes of data, be back within
~4 hours (targets, tunable). Copies stored off-site.

**The monthly restore drill.** Once a month, on purpose: restore last
night's backup to a scratch machine, replay every event, rebuild every
summary, recompute the books, compare with production. *A backup that has
never been restored is a hope, not a plan.*

**The dashboard** — six numbers, always current, watched by humans and by
the anomaly job (which can trip the auto-pause):

```
utilization %        reserve vs target %     queue length & oldest wait
average days late    liquidity runway (₱)    top 5 exposures
```

When one of these looks wrong, it looks wrong *before* members feel it.

> **Remember:** restore monthly or you don't have backups; six numbers on
> one page; the pause can pull its own trigger.

---

## Lesson 13 — Build order

```
 1. ledger/      events + books + the one-transaction rule + append-only
                 protection. The books balance from day one.
 2. policy/      versioned parameters + rule-change events (D8).
 3. onboarding   signup → OTP → KYC (photos to private storage) → active.
 4. deposits     confirm → IDLE → activation job → LOCKED (D4);
                 idle withdrawal anytime.
 5. ADVANCE      the simplest safe loan: self-collateral, freeze + consent
                 (D1), liquidity check, the FIFO queue (D10).
 6. repayments   allocation order, score + track (D5), D6 splits, the
                 dunning clock, duplicate-payment protection.
 7. payouts      monthly, strictly from collected cash (D3).
 8. COMMUNITY    vouching: pledges, consent, repeat-decay, the recovery
                 waterfall, concentration caps.
 9. risk/        the private score, fraud checks, anomaly → auto-pause,
                 the dashboard.
10. rails        Manual + Provider — both waiting on D7's legal wrapper;
                 Simulation carried the pilot until here.
11. operations   dashboards, backup + restore drills, admin dual-approval,
                 the replay-audit tool.
```

Advance ships before Community on purpose: it is the smallest product that
exercises the *entire* spine — badges, books, liquidity, queue, clock —
with almost zero credit risk.

> **Remember:** notebook first, rules second, people third, money products
> in order of risk.

---

## Lesson 14 — Boring on purpose

The guardrails that keep this system teachable:

- One program, one database — until measured pain says otherwise. Postgres
  is the queue, the scheduler, the event store, *and* the books.
- No floats near money — whole centavos only, one documented rounding rule
  (banker's rounding, applied at splits).
- The frontend never computes. `/api/v1` from day one.
- Admins are just users with extra events: dual approval above a threshold,
  no self-approval, everything logged forever.
- Simulation mode is the same engine with a fake rail — and it is the
  *only* mode that exists before the legal wrapper (D7).

Every specific number in this document — 2%/month, the ladders, the 80%
stress haircut, the 5% reserve floor, drill cadence — is an illustrative
default, marked tunable, living in `policy/`, governed by D8, and rehearsed
by Lesson 11.

---

## The final exam, one question

*"How do you know PLR never lies about money?"*

Because every action either fully happens or fully doesn't (Lesson 4);
every peso wears exactly one badge (Lesson 5); every event writes books
that must balance (Lesson 6); nothing can be edited, only reversed
(Lessons 1, 9); and every month we prove we can rebuild the whole truth
from the notebook alone (Lesson 12).

If you can say that paragraph in your own words — you pass.
