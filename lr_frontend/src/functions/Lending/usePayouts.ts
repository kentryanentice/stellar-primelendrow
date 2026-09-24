import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useSession } from '../../providers/useSession'
import { useToast } from '../../providers/useToast'
import type { Payout } from './types'
import { apiFetch } from '../apiFetch'

const API = import.meta.env.VITE_API_URL ?? ''

/** GET /payouts — reads only; `refresh` and the first-load effect apply it. */
async function fetchPayouts(signal?: AbortSignal) {
    const res = await apiFetch(`${API}/payouts`, { credentials: 'include', signal })
    if (!res.ok) throw new Error()
    const data = await res.json() as { payouts: Payout[] }
    return data.payouts
}

/**
 * The member's payouts (GET /payouts), and the two requests that start one:
 * loan proceeds (POST /loans/payout, Borrow page) and a pool withdrawal
 * (POST /pool/withdraw, Lend page). They are the same transfer with different
 * reasons, so they share this hook rather than each owning half a rail.
 *
 * The engine treats a request as an intent it owns from that moment: even if
 * PayPal is unreachable, the payout row exists and a worker retries it with
 * the same idempotency key. So a failed-looking response here never means
 * "nothing happened" — it means "not sent yet", and the status in this list
 * is the truth.
 *
 * `use`-prefixed per this repo's React Compiler requirement.
 */
export default function usePayouts() {
    const { csrfToken } = useSession()
    const toast = useToast()

    const [payouts, setPayouts] = useState<Payout[]>([])
    const [loading, setLoading] = useState(true)
    const [withdrawing, setWithdrawing] = useState(false)
    /** The withdrawal attempt still waiting for an answer, and its key. */
    const pendingWithdrawal = useRef<{ amount: number; key: string } | null>(null)

    const refresh = useCallback(async () => {
        try {
            setPayouts(await fetchPayouts())
        } catch {
            setPayouts([])
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
                const payouts = await fetchPayouts(controller.signal)
                if (!controller.signal.aborted) setPayouts(payouts)
            } catch {
                if (!controller.signal.aborted) setPayouts([])
            } finally {
                if (!controller.signal.aborted) setLoading(false)
            }
        })()
        return () => controller.abort()
    }, [])

    const authHeaders = useCallback((): HeadersInit => ({
        'Content-Type': 'application/json',
        ...(csrfToken ? { 'x-csrf-token': csrfToken } : {}),
    }), [csrfToken])

    /**
     * Takes `centavos` out of the caller's withdrawable deposit and sends it
     * to their PayPal — loan proceeds included, which land in that balance at
     * disbursement. Once the engine
     * answers at all, the withdrawal exists — a rejection here is the engine
     * refusing to start one (locked funds, no connected account), never a
     * transfer left in limbo.
     */
    const requestWithdrawal = useCallback(async (centavos: number) => {
        // One key per withdrawal attempt (engine 041). Reused only when the
        // same amount is sent again after the request never got an answer —
        // exactly the retry that must not become a second withdrawal. Any
        // answer from the engine ends the attempt.
        if (pendingWithdrawal.current?.amount !== centavos) {
            pendingWithdrawal.current = { amount: centavos, key: crypto.randomUUID() }
        }
        const requestKey = pendingWithdrawal.current.key
        setWithdrawing(true)
        try {
            const res = await apiFetch(`${API}/pool/withdraw`, {
                method: 'POST',
                credentials: 'include',
                headers: authHeaders(),
                body: JSON.stringify({ amount: centavos, request_key: requestKey }),
            })
            pendingWithdrawal.current = null
            if (!res.ok) throw new Error(await res.text() || 'Unable to withdraw')
            const data = await res.json() as { message: string }
            toast.success(data.message)
            await refresh()
            return true
        } catch (err) {
            toast.error(err instanceof Error ? err.message : 'Unable to withdraw')
            return false
        } finally {
            setWithdrawing(false)
        }
    }, [authHeaders, refresh, toast])

    /** The payout for one loan, if the member has already asked for it. */
    const forLoan = useCallback(
        (loanId: string) => payouts.find(p => p.loan_id === loanId) ?? null,
        [payouts],
    )

    /** Withdrawals only, newest first (the engine already orders the list). */
    const withdrawals = useMemo(
        () => payouts.filter(p => p.kind === 'deposit_withdrawal'),
        [payouts],
    )

    return {
        payouts, loading, refresh,
        forLoan,
        requestWithdrawal, withdrawing, withdrawals,
    }
}
