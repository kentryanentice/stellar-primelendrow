import { useCallback, useEffect, useState } from 'react'
import { useSession } from '../../providers/useSession'
import type { Lot, LotsPage } from './types'
import { apiFetch } from '../apiFetch'

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

    const fetchPage = useCallback(async (targetPage: number, signal?: AbortSignal) => {
        const res = await apiFetch(`${API}/pool/deposits`, {
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
        return await res.json() as LotsPage
    }, [csrfToken])

    const showPage = useCallback((data: LotsPage) => {
        setLots(data.items)
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

    /** Re-fetches the page currently on screen — for after a deposit/withdraw
     *  elsewhere on the page changes what this list should show. */
    const refresh = useCallback(() => load(page), [load, page])
    const goToPage = useCallback((target: number) => load(target), [load])

    return { lots, page, total, totalPages, loading, error, refresh, goToPage }
}
