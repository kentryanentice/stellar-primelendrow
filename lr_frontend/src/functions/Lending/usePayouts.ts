import { useCallback, useEffect, useMemo, useState } from 'react'
import { useSession } from '../../providers/useSession'
import { useToast } from '../../providers/useToast'
import type { Payout } from './types'

const API = import.meta.env.VITE_API_URL ?? ''

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
    const [requestingId, setRequestingId] = useState<string | null>(null)
    const [withdrawing, setWithdrawing] = useState(false)

    const refresh = useCallback(async () => {
        try {
            const res = await fetch(`${API}/payouts`, { credentials: 'include' })
            if (!res.ok) throw new Error()
            const data = await res.json() as { payouts: Payout[] }
            setPayouts(data.payouts)
        } catch {
            setPayouts([])
        } finally {
            setLoading(false)
        }
    }, [])

    useEffect(() => { void refresh() }, [refresh])

    const authHeaders = useCallback((): HeadersInit => ({
        'Content-Type': 'application/json',
        ...(csrfToken ? { 'x-csrf-token': csrfToken } : {}),
    }), [csrfToken])

    const requestPayout = useCallback(async (loanId: string) => {
        setRequestingId(loanId)
        try {
            const res = await fetch(`${API}/loans/payout`, {
                method: 'POST',
                credentials: 'include',
                headers: authHeaders(),
                body: JSON.stringify({ loan_id: loanId }),
            })
            if (!res.ok) throw new Error(await res.text() || 'Unable to send your loan to PayPal')
            const data = await res.json() as { message: string }
            toast.success(data.message)
            await refresh()
            return true
        } catch (err) {
            toast.error(err instanceof Error ? err.message : 'Unable to send your loan to PayPal')
            return false
        } finally {
            setRequestingId(null)
        }
    }, [authHeaders, refresh, toast])

    /**
     * Takes `centavos` out of the caller's withdrawable deposit and sends it
     * to their PayPal. Identical guarantees to requestPayout: once the engine
     * answers at all, the withdrawal exists — a rejection here is the engine
     * refusing to start one (locked funds, no connected account), never a
     * transfer left in limbo.
     */
    const requestWithdrawal = useCallback(async (centavos: number) => {
        setWithdrawing(true)
        try {
            const res = await fetch(`${API}/pool/withdraw`, {
                method: 'POST',
                credentials: 'include',
                headers: authHeaders(),
                body: JSON.stringify({ amount: centavos }),
            })
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
        requestPayout, requestingId, forLoan,
        requestWithdrawal, withdrawing, withdrawals,
    }
}
