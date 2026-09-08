import { useCallback, useState } from 'react'
import { useSession } from '../../providers/useSession'
import { useToast } from '../../providers/useToast'

const API = import.meta.env.VITE_API_URL ?? ''

/**
 * The reference a member presents to say "I paid" — a PayPal order id from the
 * Buttons flow, or a Stripe Checkout Session id from the redirect back. The
 * engine's `PaymentRef` accepts exactly one of the two, and which one it is
 * names the rail.
 */
export type DepositRef = { order_id: string } | { session_id: string }

/**
 * Money IN to the pool. The deposit path hands the engine nothing but the
 * reference; the engine verifies it with the provider server-side and credits
 * whatever the provider actually confirms — the amount on screen is never what
 * gets credited, the verified payment is.
 *
 * Money OUT lives in `usePayouts` (029): a withdrawal is the same tracked,
 * retried transfer as a loan payout, so it belongs with its rail rather than
 * beside the deposit form it happens to share a card with.
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

    /**
     * Called from PayPal's onApprove with the approved order id, or on return
     * from Stripe Checkout with the session id.
     *
     * Safe to call twice with the same reference: the ledger's unique rail_ref
     * refuses the second credit, so a double-submitted deposit shows an error
     * rather than crediting twice.
     */
    const confirmDeposit = useCallback(async (ref: DepositRef) => {
        setConfirming(true)
        try {
            const res = await fetch(`${API}/pool/deposit`, {
                method: 'POST',
                credentials: 'include',
                headers: authHeaders(),
                body: JSON.stringify(ref),
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
