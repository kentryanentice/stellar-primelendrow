import { pesos, rate, xlm } from '../Lending/money'
import {
    PRODUCT_LABEL,
    TRANSACTION_KIND_LABEL,
    TRANSACTION_STATUS_META,
    type Loan,
    type Payment,
    type Transaction,
    type TransactionKind,
    type TransactionTone,
} from '../Lending/types'

/**
 * Display-only derivations for the dashboard. Same rule as everything under
 * Lending: nothing here is money math the engine relies on — it picks rows
 * out of responses the engine already computed and lays them side by side.
 */

export type NextInstallment = { installment: number; total: number; dueAt: number; late: boolean }

/** The earliest not-fully-settled installment — the same derivation
 *  OpenLoanCard (Borrow) and RepayCard (Pay) use, plus whether it's late. */
export const nextInstallment = (loan: Loan): NextInstallment | null => {
    for (const row of loan.schedule) {
        const total = (row.interest_due - row.interest_paid) + (row.principal_due - row.principal_paid)
        if (total > 0) return { installment: row.installment, total, dueAt: row.due_at, late: row.status === 'late' }
    }
    return null
}

export type Standing = { label: string; cls: 'is-good' | 'is-warn' }

/**
 * The one-line answer to "am I in good standing?" for the page header. A
 * `defaulted` loan stays that status until an administrator reopens it, and a
 * `reconciling` one is still being settled — both outrank a late installment.
 * Null when there's nothing open to be on time with.
 */
export function loanStanding(loans: Loan[]): Standing | null {
    if (loans.some(l => l.status === 'defaulted' || l.status === 'reconciling')) {
        return { label: 'A loan is in default', cls: 'is-warn' }
    }
    const active = loans.filter(l => l.status === 'active')
    if (active.some(l => l.schedule.some(row => row.status === 'late'))) {
        return { label: 'A payment is late', cls: 'is-warn' }
    }
    return active.length > 0 ? { label: 'All payments on time', cls: 'is-good' } : null
}

export type ActivitySide = 'lending' | 'borrowing'
export type ActivityGlyph = 'deposit' | 'withdraw' | 'refund' | 'seized' | 'lock' | 'unlock' | 'paid' | 'disbursed'

export type ActivityRow = {
    key: string
    side: ActivitySide
    glyph: ActivityGlyph
    amount: string
    detail: string
    status: string
    tone: TransactionTone
    at: number
}

/** Pesos in and out of the pool are the lending side; XLM locked behind the
 *  member's own loan is the borrowing side. A seized deposit stays with
 *  lending — it was deposit money, whoever's loan it ended up covering. */
const SIDE: Record<TransactionKind, ActivitySide> = {
    deposit: 'lending',
    withdrawal: 'lending',
    withdrawal_refund: 'lending',
    deposit_seized: 'lending',
    collateral_lock: 'borrowing',
    collateral_release: 'borrowing',
    collateral_seize: 'borrowing',
}

const GLYPH: Record<TransactionKind, ActivityGlyph> = {
    deposit: 'deposit',
    withdrawal: 'withdraw',
    withdrawal_refund: 'refund',
    deposit_seized: 'seized',
    collateral_lock: 'lock',
    collateral_release: 'unlock',
    collateral_seize: 'seized',
}

const LOAN_STATUS: Record<Loan['status'], { label: string; tone: TransactionTone }> = {
    pending: { label: 'Pending', tone: 'is-progress' },
    active: { label: 'Active', tone: 'is-progress' },
    closed: { label: 'Repaid', tone: 'is-good' },
    reconciled: { label: 'Settled', tone: 'is-good' },
    reconciling: { label: 'Settling', tone: 'is-warn' },
    defaulted: { label: 'Defaulted', tone: 'is-warn' },
    declined: { label: 'Declined', tone: 'is-warn' },
    cancelled: { label: 'Cancelled', tone: 'is-warn' },
}

/**
 * One newest-first feed across the three places money moves for a member:
 * pool/vault movements (POST /pool/transactions), repayments (POST
 * /loans/payments) and loan disbursements (GET /loans). None of them overlap
 * — loan proceeds are a payout, not a pool transaction — so merging can't
 * double-count. Callers pass each source's first page; since every source is
 * already newest-first, the head of the merge is the true most-recent set.
 */
export function recentActivity(transactions: Transaction[], payments: Payment[], loans: Loan[]): ActivityRow[] {
    const rows: ActivityRow[] = []

    for (const tx of transactions) {
        const status = TRANSACTION_STATUS_META[tx.status]
        rows.push({
            key: `tx-${tx.id}`,
            side: SIDE[tx.kind],
            glyph: GLYPH[tx.kind],
            amount: tx.asset === 'php' ? pesos(tx.amount) : xlm(tx.amount),
            detail: TRANSACTION_KIND_LABEL[tx.kind],
            status: status.label,
            tone: status.cls,
            at: tx.at,
        })
    }

    for (const payment of payments) {
        const parts = [`${pesos(payment.principal_paid)} principal`, `${pesos(payment.interest_paid)} interest`]
        // an overpayment becomes a fresh deposit lot, so say where it went
        if (payment.excess > 0) parts.push(`${pesos(payment.excess)} to your deposits`)
        rows.push({
            key: `payment-${payment.id}`,
            side: 'borrowing',
            glyph: 'paid',
            amount: pesos(payment.amount_received),
            detail: `Payment · ${parts.join(' · ')}`,
            status: 'Paid',
            tone: 'is-good',
            at: payment.paid_at,
        })
    }

    for (const loan of loans) {
        // pending/declined/cancelled loans never moved money
        if (loan.disbursed_at === null) continue
        const status = LOAN_STATUS[loan.status]
        rows.push({
            key: `loan-${loan.id}`,
            side: 'borrowing',
            glyph: 'disbursed',
            amount: pesos(loan.principal),
            detail: `Loan disbursed · ${PRODUCT_LABEL[loan.product]} · ${rate(loan.rate_bps)}`,
            status: status.label,
            tone: status.tone,
            at: loan.disbursed_at,
        })
    }

    return rows.sort((a, b) => b.at - a.at)
}
