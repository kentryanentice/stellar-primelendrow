import { useCallback, useEffect, useState } from 'react'
import { useSession } from '../../providers/useSession'
import { useToast } from '../../providers/useToast'
import type { Loan } from './types'
import { apiFetch } from '../apiFetch'

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
    const toast = useToast()

    const [loans, setLoans] = useState<Loan[]>([])
    const [page, setPage] = useState(1)
    const [total, setTotal] = useState(0)
    const [totalPages, setTotalPages] = useState(1)
    const [loading, setLoading] = useState(true)
    /** The application a cancellation is in flight for. */
    const [cancellingId, setCancellingId] = useState<string | null>(null)
    const [error, setError] = useState(false)

    const fetchPage = useCallback(async (targetPage: number, signal?: AbortSignal) => {
        const res = await apiFetch(`${API}/loans/history`, {
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
        return await res.json() as LoansHistoryPage
    }, [csrfToken])

    const showPage = useCallback((data: LoansHistoryPage) => {
        setLoans(data.items)
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

    const refresh = useCallback(() => load(page), [load, page])
    const goToPage = useCallback((target: number) => load(target), [load])

    /**
     * Withdraws a pending application, handing the borrower back the deposit it
     * had frozen.
     *
     * The engine refuses once a guarantor has accepted — their money is behind
     * this loan and it isn't the borrower's to release — and once the coins are
     * in the vault, which needs a signed on-chain release instead. Both come
     * back as readable messages, so this hook just shows what it's told rather
     * than trying to predict either case.
     */
    const cancelLoan = useCallback(async (loanId: string) => {
        setCancellingId(loanId)
        try {
            const res = await apiFetch(`${API}/loans/cancel`, {
                method: 'POST',
                credentials: 'include',
                headers: {
                    'Content-Type': 'application/json',
                    ...(csrfToken ? { 'x-csrf-token': csrfToken } : {}),
                },
                body: JSON.stringify({ loan_id: loanId }),
            })
            if (!res.ok) throw new Error(await res.text() || 'Unable to cancel this application')
            const data = await res.json() as { message: string }
            toast.success(data.message)
            await refresh()
            return true
        } catch (err) {
            toast.error(err instanceof Error ? err.message : 'Unable to cancel this application')
            return false
        } finally {
            setCancellingId(null)
        }
    }, [csrfToken, refresh, toast])

    return { loans, page, total, totalPages, loading, error, refresh, goToPage, cancelLoan, cancellingId }
}
