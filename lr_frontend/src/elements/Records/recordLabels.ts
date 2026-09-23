import type { PublicLoanStatus, PublicRecovery, PublicScoreEvent } from '../../functions/Lending/publicLoans'

/** Pill colors, the same as the admin loan book's — a status reads the same
 *  on every screen that shows it. */
export const STATUS_CLS: Record<PublicLoanStatus, string> = {
    pending: 'is-pending',
    active: 'is-active',
    closed: 'is-closed',
    defaulted: 'is-defaulted',
    reconciling: 'is-pending',
    reconciled: 'is-closed',
    declined: 'is-declined',
    cancelled: 'is-declined',
}

/** A short, recognisable handle for a loan: the reference's first block. */
export const loanRef = (id: string) => id.slice(0, 8).toUpperCase()

/** A share of a whole, as a percent label ("12.5%"). Display only. */
export const shareOf = (part: number, whole: number) =>
    whole > 0 ? `${((part / whole) * 100).toFixed(2).replace(/\.?0+$/, '')}%` : '—'

/** Where a recovery step took its money from. Guarantors by position only. */
export function recoverySource(step: PublicRecovery) {
    switch (step.source) {
        case 'borrower_deposit': return 'Borrower’s deposit'
        case 'borrower_xlm': return 'Borrower’s XLM collateral'
        case 'guarantor_deposit': return step.guarantor ? `Guarantor ${step.guarantor}’s pledge` : 'A guarantor’s pledge'
        case 'recovery_fund': return 'Recovery fund'
        case 'reserve_fund': return 'Lending reserve'
        default: return step.source
    }
}

/** What a vault movement did, in plain words. */
export const VAULT_ACTION_LABEL: Record<string, string> = {
    mark_repaid: 'Repayment recorded on-chain',
    release: 'Collateral released to the borrower',
    mark_defaulted: 'Default recorded on-chain',
    seize: 'Collateral seized to cover the default',
}

export const COLLATERAL_STATUS_LABEL: Record<string, string> = {
    pending: 'Waiting for the lock',
    locked: 'Locked in the vault',
    released: 'Released back',
    seized: 'Seized',
}

export const PLEDGE_STATUS_LABEL: Record<string, string> = {
    invited: 'Invited',
    accepted: 'Locked',
    declined: 'Declined',
    released: 'Released',
    seized: 'Seized',
}

export const INSTALLMENT_STATUS_LABEL: Record<string, string> = {
    scheduled: 'Scheduled',
    paid: 'Paid',
    late: 'Late',
    defaulted: 'Defaulted',
}

/** Why a score moved, keyed by the engine's reason code. */
export const SCORE_REASON_LABEL: Record<string, string> = {
    loan_repaid_term_complete: 'Repaid in full, term completed',
    loan_defaulted: 'Loan defaulted',
    default_settled: 'Default settled',
    guarantor_claimed: 'Pledge claimed on default',
    guarantor_claim_settled: 'Claimed pledge settled',
}

/** Why a score moved. A rise still waiting on its term says so, rather than
 *  reading as a term already complete. */
export function scoreReason(event: PublicScoreEvent) {
    if (event.status === 'pending') return 'Repaid in full, term still running'
    return event.reason ? SCORE_REASON_LABEL[event.reason] ?? event.reason : '—'
}

/** The score a change moved (or, while pending, is expected to move) from and
 *  to: "60 → 65". Null when there is none to show. */
export const scoreRange = (event: PublicScoreEvent) =>
    event.score_from !== null && event.score_to !== null ? `${event.score_from} → ${event.score_to}` : null

/** Whose record moved. Guarantors by position only. */
export const scoreSubject = (event: PublicScoreEvent) =>
    event.subject === 'borrower' ? 'Borrower' : event.position ? `Guarantor ${event.position}` : 'Guarantor'

/** A score movement with its sign: "+5", "−25". */
export const points = (delta: number) => (delta > 0 ? `+${delta}` : delta < 0 ? `−${-delta}` : '0')
