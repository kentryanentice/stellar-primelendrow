import { useCallback, useEffect, useState } from 'react'
import { useSession } from '../../providers/useSession'
import type { Transaction, TransactionsPage } from './types'
import { apiFetch } from '../apiFetch'

const API = import.meta.env.VITE_API_URL ?? ''

/**
 * The caller's money movements (POST /pool/transactions), paginated — deposits
 * and withdrawals in pesos, collateral locks, releases and seizures in XLM,
 * already interleaved and ordered by the engine.
 *
 * Same shape as useDeposits deliberately: that list shows lots that still
 * exist, this one shows what happened. A withdrawal deletes the lots it
 * consumes, so the deposits list can't be the history and never could be.
 *
 * `use`-prefixed per this repo's React Compiler requirement.
 */
export default function useTransactions() {
    const { csrfToken } = useSession()

    const [items, setItems] = useState<Transaction[]>([])
    const [page, setPage] = useState(1)
    const [total, setTotal] = useState(0)
    const [totalPages, setTotalPages] = useState(1)
    const [loading, setLoading] = useState(true)
    const [error, setError] = useState(false)

    const fetchPage = useCallback(async (targetPage: number, signal?: AbortSignal) => {
        const res = await apiFetch(`${API}/pool/transactions`, {
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
        return await res.json() as TransactionsPage
    }, [csrfToken])

    const showPage = useCallback((data: TransactionsPage) => {
        setItems(data.items)
        setPage(data.page)
        setTotal(data.total)
        setTotalPages(data.total_pages)
    }, [])

    const load = useCallback(async (targetPage: number) => {
        setLoading(true)
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

    /** Re-fetches the page on screen — for after a deposit or withdrawal
     *  elsewhere on the page adds a row to it. */
    const refresh = useCallback(() => load(page), [load, page])
    const goToPage = useCallback((target: number) => load(target), [load])

    return { items, page, total, totalPages, loading, error, refresh, goToPage }
}
