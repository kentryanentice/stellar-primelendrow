import { useCallback, useEffect, useState } from 'react'
import { useSession } from '../../providers/useSession'
import type { Payment } from './types'
import { apiFetch } from '../apiFetch'

const API = import.meta.env.VITE_API_URL ?? ''

export type PaymentTotals = { amount_received: number; interest_paid: number; principal_paid: number; fee_paid: number }

type PaymentsPage = {
    items: Payment[]
    total: number
    page: number
    page_size: number
    total_pages: number
    totals: PaymentTotals
}

const ZERO_TOTALS: PaymentTotals = { amount_received: 0, interest_paid: 0, principal_paid: 0, fee_paid: 0 }

/**
 * The caller's repayment history (POST /loans/payments), paginated, plus
 * all-time totals across every payment (not just the page on screen) for the
 * "Repaid to date" hero. `use`-prefixed per this repo's React Compiler
 * requirement.
 */
export default function usePayments() {
    const { csrfToken } = useSession()

    const [payments, setPayments] = useState<Payment[]>([])
    const [page, setPage] = useState(1)
    const [total, setTotal] = useState(0)
    const [totalPages, setTotalPages] = useState(1)
    const [totals, setTotals] = useState<PaymentTotals>(ZERO_TOTALS)
    const [loading, setLoading] = useState(true)
    const [error, setError] = useState(false)

    const fetchPage = useCallback(async (targetPage: number, signal?: AbortSignal) => {
        const res = await apiFetch(`${API}/loans/payments`, {
            method: 'POST',
            credentials: 'include',
            headers: {
                'Content-Type': 'application/json',
                ...(csrfToken ? { 'x-csrf-token': csrfToken } : {}),
            },
            body: JSON.stringify({ page: targetPage }),
            signal,
        })
        if (!res.ok) throw new Error()
        return await res.json() as PaymentsPage
    }, [csrfToken])

    const showPage = useCallback((data: PaymentsPage) => {
        setPayments(data.items)
        setPage(data.page)
        setTotal(data.total)
        setTotalPages(data.total_pages)
        setTotals(data.totals)
    }, [])

    // `quiet` keeps the rows on screen while they reload — used after a
    // payment, where the list is only gaining a row and swapping it for a
    // skeleton would read as the history disappearing.
    const load = useCallback(async (targetPage: number, quiet = false) => {
        if (!quiet) setLoading(true)
        setError(false)
        try {
            showPage(await fetchPage(targetPage))
        } catch {
            setError(true)
        } finally {
            setLoading(false)
        }
    }, [fetchPage, showPage])

    // A new CSRF token reloads page 1 (the effect below). Flag that during
    // render — React's way of adjusting state to a changed input — so the
    // effect itself never has to set state before the response is in.
    const [loadedFor, setLoadedFor] = useState(csrfToken)
    if (loadedFor !== csrfToken) {
        setLoadedFor(csrfToken)
        setLoading(true)
        setError(false)
    }

    useEffect(() => {
        const controller = new AbortController()
        void (async () => {
            try {
                const data = await fetchPage(1, controller.signal)
                if (!controller.signal.aborted) showPage(data)
            } catch {
                if (!controller.signal.aborted) setError(true)
            } finally {
                if (!controller.signal.aborted) setLoading(false)
            }
        })()
        return () => controller.abort()
    }, [fetchPage, showPage])

    const refresh = useCallback(() => load(page, true), [load, page])
    const goToPage = useCallback((target: number) => load(target), [load])

    return { payments, page, total, totalPages, totals, loading, error, refresh, goToPage }
}
