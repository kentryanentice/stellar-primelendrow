import { useCallback, useEffect, useState } from 'react'
import { useSession } from '../../providers/useSession'
import type { Payment } from './types'

const API = import.meta.env.VITE_API_URL ?? ''

export type PaymentTotals = { amount_received: number; interest_paid: number; principal_paid: number }

type PaymentsPage = {
    items: Payment[]
    total: number
    page: number
    page_size: number
    total_pages: number
    totals: PaymentTotals
}

const ZERO_TOTALS: PaymentTotals = { amount_received: 0, interest_paid: 0, principal_paid: 0 }

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

    const load = useCallback(async (targetPage: number) => {
        setLoading(true)
        setError(false)
        try {
            const res = await fetch(`${API}/loans/payments`, {
                method: 'POST',
                credentials: 'include',
                headers: {
                    'Content-Type': 'application/json',
                    ...(csrfToken ? { 'x-csrf-token': csrfToken } : {}),
                },
                body: JSON.stringify({ page: targetPage }),
            })
            if (!res.ok) throw new Error()
            const data = await res.json() as PaymentsPage
            setPayments(data.items)
            setPage(data.page)
            setTotal(data.total)
            setTotalPages(data.total_pages)
            setTotals(data.totals)
        } catch {
            setError(true)
        } finally {
            setLoading(false)
        }
    }, [csrfToken])

    useEffect(() => { void load(1) }, [load])

    const refresh = useCallback(() => load(page), [load, page])
    const goToPage = useCallback((target: number) => load(target), [load])

    return { payments, page, total, totalPages, totals, loading, error, refresh, goToPage }
}
