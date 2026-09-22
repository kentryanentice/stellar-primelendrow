import { pesos } from './money'
import type { Payment, Transaction } from './types'

export type ReceiptLine = {
    label: string
    value: string
    /** What the movement came down to — the line a member checks first. */
    total?: boolean
    /** A fee, a deduction or a breakdown line: quieter than the amounts. */
    muted?: boolean
}

/** Stripe capture references are stamped `stripe:<payment intent>` by the
 *  engine (`infra::stripe`); everything else that pays in is PayPal. */
const payInFeeLabel = (reference: string | null) =>
    reference?.startsWith('stripe:') ? 'Stripe fee' : 'PayPal fee'

/**
 * The receipt for a deposit or withdrawal: what was charged, what the provider
 * kept, and what actually landed.
 *
 * Built only from numbers the engine recorded when the money moved — `paid`,
 * `fee`, `received`, `amount` — and never re-estimated from today's fee
 * policy. A receipt that recomputed the fee would drift the moment the rates
 * changed, and would show a member a figure they were never charged.
 *
 * Null for movements with nothing to break down: collateral (priced in XLM,
 * with its own on-chain record), interest earned, and returned withdrawals.
 */
export function transactionReceipt(tx: Transaction): ReceiptLine[] | null {
    if (tx.asset !== 'php') return null

    if (tx.kind === 'deposit') {
        const lines: ReceiptLine[] = []
        // Deposits before fees were passed on (043) carry no `paid`; for them
        // what was paid and what was credited were the same number.
        if (tx.paid !== null && tx.fee) {
            lines.push({ label: 'You paid', value: pesos(tx.paid) })
            lines.push({ label: payInFeeLabel(tx.reference), value: pesos(tx.fee), muted: true })
        }
        lines.push({ label: 'Credited to your balance', value: pesos(tx.amount), total: true })
        return lines
    }

    if (tx.kind === 'withdrawal') {
        const lines: ReceiptLine[] = [{ label: 'Taken from your balance', value: pesos(tx.amount) }]
        // The payout fee comes OUT of the amount (043), so it reads as a
        // deduction here rather than as something added on top.
        if (tx.fee) lines.push({ label: 'Payout fee', value: `− ${pesos(tx.fee)}`, muted: true })
        if (tx.received !== null) {
            lines.push({ label: 'Sent to you', value: pesos(tx.received), total: true })
        }
        return lines
    }

    return null
}

/**
 * The receipt for one repayment: what the borrower paid, the fee on top, and
 * how what reached the loan split between principal and interest.
 *
 * `amount_received` is what was applied; `fee_paid` was paid on top of it
 * (engine 043), so the two together are what the borrower was charged.
 */
export function paymentReceipt(p: Payment): ReceiptLine[] {
    const lines: ReceiptLine[] = []
    if (p.fee_paid > 0) {
        lines.push({ label: 'You paid', value: pesos(p.amount_received + p.fee_paid) })
        lines.push({ label: 'Payment fee', value: pesos(p.fee_paid), muted: true })
    }
    lines.push({ label: 'Applied to your loan', value: pesos(p.principal_paid + p.interest_paid), total: true })
    lines.push({ label: 'Principal', value: pesos(p.principal_paid), muted: true })
    lines.push({ label: 'Interest', value: pesos(p.interest_paid), muted: true })
    // Repayments have been exact since 041, so this only appears on older rows
    // where an overpayment became a fresh deposit.
    if (p.excess > 0) {
        lines.push({ label: 'Returned to your balance', value: pesos(p.excess), muted: true })
    }
    return lines
}
