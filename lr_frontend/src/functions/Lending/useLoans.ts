import { useCallback, useEffect, useState } from 'react'
import { useSession } from '../../providers/useSession'
import { useToast } from '../../providers/useToast'
import type { Loan } from './types'

const API = import.meta.env.VITE_API_URL ?? ''

/**
 * The caller's loans with their engine-pinned schedules, plus repayment
 * (PayPal order id in, engine allocates interest-then-principal out).
 * `use`-prefixed per this repo's React Compiler requirement.
 */
export default function useLoans() {
    const { csrfToken } = useSession()
    const toast = useToast()

    const [loans, setLoans] = useState<Loan[]>([])
    const [loading, setLoading] = useState(true)
    const [error, setError] = useState(false)
    const [repayingId, setRepayingId] = useState<string | null>(null)

    const refresh = useCallback(async () => {
        setError(false)
        try {
            const res = await fetch(`${API}/loans`, { credentials: 'include' })
            if (!res.ok) throw new Error()
            const data = await res.json() as { loans: Loan[] }
            setLoans(data.loans)
        } catch {
            setError(true)
        } finally {
            setLoading(false)
        }
    }, [])

    useEffect(() => {
        void refresh()
    }, [refresh])

    /** Called from PayPal's onApprove with the approved order id. */
    const repay = useCallback(async (loanId: string, orderId: string) => {
        setRepayingId(loanId)
        try {
            const res = await fetch(`${API}/loans/repay`, {
                method: 'POST',
                credentials: 'include',
                headers: {
                    'Content-Type': 'application/json',
                    ...(csrfToken ? { 'x-csrf-token': csrfToken } : {}),
                },
                body: JSON.stringify({ loan_id: loanId, order_id: orderId }),
            })
            if (!res.ok) throw new Error(await res.text() || 'Unable to apply your payment')
            const data = await res.json() as { message: string }
            toast.success(data.message)
            await refresh()
            return true
        } catch (err) {
            toast.error(err instanceof Error ? err.message : 'Unable to apply your payment')
            return false
        } finally {
            setRepayingId(null)
        }
    }, [csrfToken, refresh, toast])

    return { loans, loading, error, refresh, repay, repayingId }
}
