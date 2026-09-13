import { useCallback, useEffect, useState } from 'react'
import { useSession } from '../../providers/useSession'
import { useToast } from '../../providers/useToast'
import type { Loan } from './types'
import { apiFetch } from '../apiFetch'

const API = import.meta.env.VITE_API_URL ?? ''

/** GET /loans — reads only; `refresh` and the first-load effect apply it. */
async function fetchLoans(signal?: AbortSignal) {
    const res = await apiFetch(`${API}/loans`, { credentials: 'include', signal })
    if (!res.ok) throw new Error()
    const data = await res.json() as { loans: Loan[] }
    return data.loans
}

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
            setLoans(await fetchLoans())
        } catch {
            setError(true)
        } finally {
            setLoading(false)
        }
    }, [])

    // First load. The initial state already says "loading", so state is only
    // written once the response is in — never synchronously in the effect.
    useEffect(() => {
        const controller = new AbortController()
        void (async () => {
            try {
                const loans = await fetchLoans(controller.signal)
                if (!controller.signal.aborted) setLoans(loans)
            } catch {
                if (!controller.signal.aborted) setError(true)
            } finally {
                if (!controller.signal.aborted) setLoading(false)
            }
        })()
        return () => controller.abort()
    }, [])

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
            const res = await apiFetch(`${API}/loans/repay`, {
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
