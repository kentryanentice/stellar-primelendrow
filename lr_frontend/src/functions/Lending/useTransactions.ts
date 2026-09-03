import { useCallback, useEffect, useState } from 'react'
import { useSession } from '../../providers/useSession'
import type { Transaction, TransactionsPage } from './types'

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

    const load = useCallback(async (targetPage: number) => {
        setLoading(true)
        setError(false)
        try {
            const res = await fetch(`${API}/pool/transactions`, {
                method: 'POST',
                credentials: 'include',
                headers: {
                    'Content-Type': 'application/json',
                    ...(csrfToken ? { 'x-csrf-token': csrfToken } : {}),
                },
                body: JSON.stringify({ page: targetPage }),
            })
            if (!res.ok) throw new Error()
            const data = await res.json() as TransactionsPage
            setItems(data.items)
            setPage(data.page)
            setTotal(data.total)
            setTotalPages(data.total_pages)
        } catch {
            setError(true)
        } finally {
            setLoading(false)
        }
    }, [csrfToken])

    useEffect(() => { void load(1) }, [load])

    /** Re-fetches the page on screen — for after a deposit or withdrawal
     *  elsewhere on the page adds a row to it. */
    const refresh = useCallback(() => load(page), [load, page])
    const goToPage = useCallback((target: number) => load(target), [load])

    return { items, page, total, totalPages, loading, error, refresh, goToPage }
}
