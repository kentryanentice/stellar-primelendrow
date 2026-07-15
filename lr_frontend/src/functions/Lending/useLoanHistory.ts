import { useCallback, useEffect, useState } from 'react'
import { useSession } from '../../providers/useSession'
import type { Loan } from './types'

const API = import.meta.env.VITE_API_URL ?? ''

type LoansHistoryPage = {
    items: Loan[]
    total: number
    page: number
    page_size: number
    total_pages: number
}

/**
 * The caller's own loans (POST /loans/history), paginated — the "Your loans"
 * card on the Borrow page. Separate from GET /loans (unpaginated, used by the
 * Pay page to find the one loan that can ever be pending/active at a time) —
 * this is the full closed-loan history, which only grows.
 * `use`-prefixed per this repo's React Compiler requirement.
 */
export default function useLoanHistory() {
    const { csrfToken } = useSession()

    const [loans, setLoans] = useState<Loan[]>([])
    const [page, setPage] = useState(1)
    const [total, setTotal] = useState(0)
    const [totalPages, setTotalPages] = useState(1)
    const [loading, setLoading] = useState(true)
    const [error, setError] = useState(false)

    const load = useCallback(async (targetPage: number) => {
        setLoading(true)
        setError(false)
        try {
            const res = await fetch(`${API}/loans/history`, {
                method: 'POST',
                credentials: 'include',
                headers: {
                    'Content-Type': 'application/json',
                    ...(csrfToken ? { 'x-csrf-token': csrfToken } : {}),
                },
                body: JSON.stringify({ page: targetPage }),
            })
            if (!res.ok) throw new Error()
            const data = await res.json() as LoansHistoryPage
            setLoans(data.items)
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

    const refresh = useCallback(() => load(page), [load, page])
    const goToPage = useCallback((target: number) => load(target), [load])

    return { loans, page, total, totalPages, loading, error, refresh, goToPage }
}
