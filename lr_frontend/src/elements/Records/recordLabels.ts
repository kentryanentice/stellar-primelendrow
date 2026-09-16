import type { PublicLoanStatus, PublicRecovery } from '../../functions/Lending/publicLoans'

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
