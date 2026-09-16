import { useEffect, useState } from 'react'
import { apiFetch } from '../apiFetch'
import type { InterestParts, Product } from './types'

const API = import.meta.env.VITE_API_URL ?? ''

/**
 * The public loan book (engine `lending::public`): every loan application
 * with nobody in it. Mirrors the engine's views field for field — there is no
 * borrower, guarantor or depositor identity in any of these types because the
 * engine never sends one; a guarantor is a position on the loan and depositors
 * are a count.
 */

export type PublicLoanStatus =
    | 'pending' | 'active' | 'closed' | 'defaulted' | 'declined' | 'cancelled' | 'reconciling' | 'reconciled'

export type PublicLoan = {
    id: string
    product: Product
    status: PublicLoanStatus
    principal: number
    principal_outstanding: number
    rate_bps: number
    term_months: number
    applied_at: number
    updated_at: number
    disbursed_at: number | null
    closed_at: number | null
    defaulted_at: number | null
    reconciled_at: number | null
    deposit_locked: number
    xlm_required_stroops: number | null
    xlm_locked_stroops: number | null
    guarantor_locked: number
    guarantors: number
    payments: number
    repaid: number
    interest_paid: number
}

export type PublicSummary = {
    loans: number
    by_status: { status: PublicLoanStatus; count: number }[]
    disbursed: number
    repaid: number
    interest: number
}

export type PublicLoansPage = {
    items: PublicLoan[]
    total: number
    page: number
    page_size: number
    total_pages: number
    summary: PublicSummary
}

export type PublicVaultAction = {
    action: string
    status: 'queued' | 'done'
    tx_hash: string | null
    at: number
    moved_stroops: number | null
    value_centavos: number | null
}

export type PublicCollateral = {
    required_stroops: number
    locked_stroops: number
    status: 'pending' | 'locked' | 'released' | 'seized'
    lock_tx_hash: string | null
    locked_at: number | null
    collateral_ratio_bps: number
    covers_centavos: number | null
    actions: PublicVaultAction[]
}

export type PublicGuarantor = {
    position: number
    pledge_amount: number
    status: 'invited' | 'accepted' | 'declined' | 'released' | 'seized'
}

export type PublicSplit = {
    interest: number
    parts: InterestParts
    policy_version: number | null
    pool_balance: number | null
    pledged_total: number | null
    depositors_paid: number
    guarantors: { position: number | null; tier_share: number | null; amount: number }[]
}

export type PublicPayment = {
    paid_at: number
    amount: number
    principal_paid: number
    interest_paid: number
    fee_paid: number
    split: PublicSplit | null
}

export type PublicRecovery = {
    step: number
    source: 'borrower_deposit' | 'borrower_xlm' | 'guarantor_deposit' | 'recovery_fund' | 'reserve_fund'
    guarantor: number | null
    amount: number
    refunded: number
    stroops: number | null
    at: number
}

export type PublicInstallment = {
    installment: number
    due_at: number
    principal_due: number
    interest_due: number
    principal_paid: number
    interest_paid: number
    status: 'scheduled' | 'paid' | 'late' | 'defaulted'
}

export type PublicLoanDetail = {
    loan: PublicLoan
    policy_version: number
    borrower_cover: number
    pool_funded: number | null
    locked_now: { collateral: number; lent: number; pledged: number }
    collateral: PublicCollateral | null
    guarantors: PublicGuarantor[]
    schedule: PublicInstallment[]
    payments: PublicPayment[]
    recoveries: PublicRecovery[]
}

/** The list filters, exactly as the engine whitelists them. `''` is "all". */
export type StatusFilter = '' | 'pending' | 'active' | 'closed' | 'defaulted' | 'declined' | 'cancelled'
export type ProductFilter = '' | Product

/** A status in plain words. `closed` is a loan that was paid off. */
export const PUBLIC_STATUS_LABEL: Record<PublicLoanStatus, string> = {
    pending: 'Pending',
    active: 'Active',
    closed: 'Repaid',
    defaulted: 'Defaulted',
    declined: 'Declined',
    cancelled: 'Cancelled',
    reconciling: 'Settling',
    reconciled: 'Settled',
}

class NotFound extends Error {}

/** No session, no cookie: this is the one read anybody can make. */
async function getJson<T>(path: string, signal: AbortSignal): Promise<T> {
    const res = await apiFetch(`${API}${path}`, { signal })
    if (res.status === 404) throw new NotFound()
    if (!res.ok) throw new Error()
    return await res.json() as T
}

type Loaded<T> = { key: string; data: T | null; failed: 'error' | 'not_found' | null }

/**
 * Loads `path` whenever it changes. Loading is derived — the result on hand is
 * for a different path — so the effect only ever sets state once a response
 * (or a failure) is in, never synchronously.
 *
 * `keepPrevious` leaves the last good data on screen while the next path
 * loads — a page turn in the list, where blanking the rows would read as the
 * book emptying. A different loan's record never shows the previous one.
 */
function usePublicRead<T>(path: string | null, keepPrevious = false) {
    const [result, setResult] = useState<Loaded<T> | null>(null)

    useEffect(() => {
        if (path === null) return
        const controller = new AbortController()
        void (async () => {
            try {
                const data = await getJson<T>(path, controller.signal)
                if (!controller.signal.aborted) setResult({ key: path, data, failed: null })
            } catch (err) {
                if (!controller.signal.aborted) {
                    setResult({ key: path, data: null, failed: err instanceof NotFound ? 'not_found' : 'error' })
                }
            }
        })()
        return () => controller.abort()
    }, [path])

    const current = result && result.key === path ? result : null
    return {
        data: current ? current.data : keepPrevious ? result?.data ?? null : null,
        loading: path !== null && current === null,
        error: current?.failed === 'error',
        notFound: current?.failed === 'not_found',
    }
}

/** GET /public/loans — one page of the book, filtered, plus the book's totals. */
export function usePublicLoans(page: number, status: StatusFilter, product: ProductFilter) {
    const query = new URLSearchParams({ page: String(page) })
    if (status) query.set('status', status)
    if (product) query.set('product', product)
    return usePublicRead<PublicLoansPage>(`/public/loans?${query.toString()}`, true)
}

/** GET /public/loans/{id} — one loan's whole record. */
export function usePublicLoan(loanId: string | undefined) {
    // A malformed reference can't name a loan; don't ask the engine about it.
    const valid = loanId !== undefined && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(loanId)
    const read = usePublicRead<PublicLoanDetail>(valid ? `/public/loans/${loanId}` : null)
    return { ...read, notFound: read.notFound || !valid }
}
