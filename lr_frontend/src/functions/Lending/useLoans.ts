import { useCallback, useEffect, useState } from 'react'
import { useSession } from '../../providers/useSession'
import { useToast } from '../../providers/useToast'
import type { Loan } from './types'

const API = import.meta.env.VITE_API_URL ?? ''

/**
 * The reference a borrower presents to say "I paid" — the repayment twin of
 * `DepositRef`. Exactly one of the two, and which one it is names the rail.
 */
export type RepayRef = { order_id: string } | { session_id: string }

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

    /**
     * Applies a verified payment to a loan.
     *
     * Takes the same `PaymentRef` shape the deposit path does — a PayPal order
     * id from the Buttons flow, or a Stripe Checkout Session id from the
     * redirect back. The engine accepts exactly one of the two and asks that
     * provider what was really paid, so which rail it came from changes
     * nothing about how the money is verified.
     */
    const repay = useCallback(async (loanId: string, ref: RepayRef) => {
        setRepayingId(loanId)
        try {
            const res = await fetch(`${API}/loans/repay`, {
                method: 'POST',
                credentials: 'include',
                headers: {
                    'Content-Type': 'application/json',
                    ...(csrfToken ? { 'x-csrf-token': csrfToken } : {}),
                },
                body: JSON.stringify({ loan_id: loanId, ...ref }),
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
