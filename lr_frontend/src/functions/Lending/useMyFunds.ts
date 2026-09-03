import { useCallback, useState } from 'react'
import { useSession } from '../../providers/useSession'
import { useToast } from '../../providers/useToast'

const API = import.meta.env.VITE_API_URL ?? ''

/**
 * Money IN to the pool. The deposit path receives a PayPal order id from the
 * Buttons flow and hands it to the engine, which captures server-side and
 * credits whatever PayPal actually confirms — the amount on screen is never
 * what gets credited, the capture is.
 *
 * Money OUT lives in `usePayouts` (029): a withdrawal is the same tracked,
 * retried PayPal transfer as a loan payout, so it belongs with its rail rather
 * than beside the deposit form it happens to share a card with.
 *
 * `use`-prefixed per this repo's React Compiler requirement.
 */
export default function useMyFunds(onChanged: () => void) {
    const { csrfToken } = useSession()
    const toast = useToast()

    const [confirming, setConfirming] = useState(false)

    const authHeaders = useCallback((): HeadersInit => ({
        'Content-Type': 'application/json',
        ...(csrfToken ? { 'x-csrf-token': csrfToken } : {}),
    }), [csrfToken])

    /** Called from PayPal's onApprove with the approved order id. */
    const confirmDeposit = useCallback(async (orderId: string) => {
        setConfirming(true)
        try {
            const res = await fetch(`${API}/pool/deposit`, {
                method: 'POST',
                credentials: 'include',
                headers: authHeaders(),
                body: JSON.stringify({ order_id: orderId }),
            })
            if (!res.ok) throw new Error(await res.text() || 'Unable to confirm your deposit')
            const data = await res.json() as { message: string }
            toast.success(data.message)
            onChanged()
        } catch (err) {
            toast.error(err instanceof Error ? err.message : 'Unable to confirm your deposit')
        } finally {
            setConfirming(false)
        }
    }, [authHeaders, onChanged, toast])

    return { confirmDeposit, confirming }
}
