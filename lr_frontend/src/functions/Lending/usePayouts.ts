import { useCallback, useEffect, useState } from 'react'
import { useSession } from '../../providers/useSession'
import { useToast } from '../../providers/useToast'
import type { Payout } from './types'

const API = import.meta.env.VITE_API_URL ?? ''

/**
 * The member's payouts (GET /payouts), and the request that starts one
 * (POST /loans/payout).
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

    const requestPayout = useCallback(async (loanId: string) => {
        setRequestingId(loanId)
        try {
            const res = await fetch(`${API}/loans/payout`, {
                method: 'POST',
                credentials: 'include',
                headers: {
                    'Content-Type': 'application/json',
                    ...(csrfToken ? { 'x-csrf-token': csrfToken } : {}),
                },
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
    }, [csrfToken, refresh, toast])

    /** The payout for one loan, if the member has already asked for it. */
    const forLoan = useCallback(
        (loanId: string) => payouts.find(p => p.loan_id === loanId) ?? null,
        [payouts],
    )

    return { payouts, loading, requestingId, requestPayout, forLoan, refresh }
}
