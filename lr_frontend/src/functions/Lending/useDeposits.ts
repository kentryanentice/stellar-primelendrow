import { useCallback, useEffect, useState } from 'react'
import { useSession } from '../../providers/useSession'
import type { Lot, LotsPage } from './types'

const API = import.meta.env.VITE_API_URL ?? ''

/**
 * The caller's own deposit lots (POST /pool/deposits), paginated — the "Your
 * deposits" list. Split out from GET /pool, which only carries the badge
 * totals now. `use`-prefixed per this repo's React Compiler requirement.
 */
export default function useDeposits() {
    const { csrfToken } = useSession()

    const [lots, setLots] = useState<Lot[]>([])
    const [page, setPage] = useState(1)
    const [total, setTotal] = useState(0)
    const [totalPages, setTotalPages] = useState(1)
    const [loading, setLoading] = useState(true)
    const [error, setError] = useState(false)

    const load = useCallback(async (targetPage: number) => {
        setLoading(true)
        setError(false)
        try {
            const res = await fetch(`${API}/pool/deposits`, {
                method: 'POST',
                credentials: 'include',
                headers: {
                    'Content-Type': 'application/json',
                    ...(csrfToken ? { 'x-csrf-token': csrfToken } : {}),
                },
                body: JSON.stringify({ page: targetPage }),
            })
            if (!res.ok) throw new Error()
            const data = await res.json() as LotsPage
            setLots(data.items)
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

    /** Re-fetches the page currently on screen — for after a deposit/withdraw
     *  elsewhere on the page changes what this list should show. */
    const refresh = useCallback(() => load(page), [load, page])
    const goToPage = useCallback((target: number) => load(target), [load])

    return { lots, page, total, totalPages, loading, error, refresh, goToPage }
}
