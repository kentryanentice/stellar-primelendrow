import { useCallback, useEffect, useState } from 'react'
import { useSession } from '../../providers/useSession'
import type { InterestParts } from '../Lending/types'
import { apiFetch } from '../apiFetch'

const API = import.meta.env.VITE_API_URL ?? ''

/** One member paid from a repayment's interest. */
export type InterestRecipient = {
    username: string
    role: 'depositor' | 'guarantor'
    /** What the slice was proportional to: the deposit balance for a
     *  depositor, the pledge for a guarantor. */
    weight: number
    /** The tier percent a guarantor was paid at. */
    tier_share: number | null
    amount: number
}

/** One repayment's interest and everyone it reached. */
export type InterestPayment = {
    event_id: number
    loan_id: string
    product: 'deposit_backed' | 'xlm_collateral' | 'guarantor'
    borrower: string
    paid_at: number
    interest: number
    parts: InterestParts
    /** The pool's deposit balance the depositors' share was divided by. Null
     *  on repayments recorded before it was captured. */
    pool_balance: number | null
    /** The loan's accepted pledges the guarantors' share was weighted by. */
    pledged_total: number | null
    policy_version: number | null
    recipients: InterestRecipient[]
}

type InterestPage = {
    items: InterestPayment[]
    total: number
    page: number
    page_size: number
    total_pages: number
}

/**
 * The operator's interest history (POST /lending/admin/interest): every
 * repayment's split and the members paid from it, newest first, optionally
 * narrowed to a username. Read-only. `use`-prefixed per this repo's React
 * Compiler requirement.
 */
export default function useAdminInterest() {
    const { csrfToken } = useSession()

    const [payments, setPayments] = useState<InterestPayment[]>([])
    const [page, setPage] = useState(1)
    const [total, setTotal] = useState(0)
    const [totalPages, setTotalPages] = useState(1)
    const [search, setSearch] = useState('')
    const [loading, setLoading] = useState(true)
    const [error, setError] = useState(false)

    const fetchPage = useCallback(async (targetPage: number, term: string, signal?: AbortSignal) => {
        const res = await apiFetch(`${API}/lending/admin/interest`, {
            method: 'POST',
            credentials: 'include',
            headers: {
                'Content-Type': 'application/json',
                ...(csrfToken ? { 'x-csrf-token': csrfToken } : {}),
            },
            body: JSON.stringify({ page: targetPage, search: term }),
            signal,
        })
        if (!res.ok) throw new Error()
        return await res.json() as InterestPage
    }, [csrfToken])

    const show = useCallback((data: InterestPage) => {
        setPayments(data.items)
        setPage(data.page)
        setTotal(data.total)
        setTotalPages(data.total_pages)
    }, [])

    const load = useCallback(async (targetPage: number) => {
        setLoading(true)
        setError(false)
        try {
            show(await fetchPage(targetPage, search))
        } catch {
            setError(true)
        } finally {
            setLoading(false)
        }
    }, [fetchPage, search, show])

    // A new search (or CSRF token) reloads page 1 below. Flag loading during
    // render so the previous results never sit under the new search term.
    const key = `${search}|${csrfToken ?? ''}`
    const [loadedFor, setLoadedFor] = useState(key)
    if (loadedFor !== key) {
        setLoadedFor(key)
        setLoading(true)
        setError(false)
    }

    useEffect(() => {
        const controller = new AbortController()
        void (async () => {
            try {
                const data = await fetchPage(1, search, controller.signal)
                if (!controller.signal.aborted) show(data)
            } catch {
                if (!controller.signal.aborted) setError(true)
            } finally {
                if (!controller.signal.aborted) setLoading(false)
            }
        })()
        return () => controller.abort()
    }, [fetchPage, search, show])

    const refresh = useCallback(() => load(page), [load, page])
    const goToPage = useCallback((target: number) => load(target), [load])

    return { payments, page, total, totalPages, search, setSearch, loading, error, refresh, goToPage }
}
