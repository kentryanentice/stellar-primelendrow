import { useCallback, useState } from 'react'
import { useSession } from '../../providers/useSession'
import { useToast } from '../../providers/useToast'

const API = import.meta.env.VITE_API_URL ?? ''

/** What a Stripe payment is for. Mirrors the engine's whitelist. */
export type CheckoutPurpose = 'deposit' | 'repay'

/**
 * Money IN over the Stripe rail.
 *
 * Structurally different from `usePayPal` in one important way: there is no
 * SDK to load and no client-side order to create. The engine creates the
 * Checkout Session — deciding the amount, and stamping the paying member into
 * it — and this hook does nothing but ask for the session and navigate to it.
 * Nothing about the payment is decided in the browser, so there is nothing in
 * this file for a tampered page to change.
 *
 * The trade is a redirect: the member leaves the app to pay and comes back
 * through `success_url`. That is why `stripeCheckoutResult` exists, and why
 * the confirming call happens on page load rather than in an onApprove
 * callback.
 *
 * `use`-prefixed per this repo's React Compiler requirement.
 */
export default function useStripeCheckout() {
    const { csrfToken } = useSession()
    const toast = useToast()

    const [starting, setStarting] = useState(false)

    /**
     * Sends the member to Stripe's hosted payment page. Resolves only if it
     * *failed* — on success the page is already navigating away.
     */
    const startCheckout = useCallback(async (
        purpose: CheckoutPurpose,
        centavos: number,
        loanId?: string,
    ) => {
        setStarting(true)
        try {
            const res = await fetch(`${API}/stripe/checkout`, {
                method: 'POST',
                credentials: 'include',
                headers: {
                    'Content-Type': 'application/json',
                    ...(csrfToken ? { 'x-csrf-token': csrfToken } : {}),
                },
                body: JSON.stringify({
                    purpose,
                    amount: centavos,
                    ...(loanId ? { loan_id: loanId } : {}),
                }),
            })
            if (!res.ok) throw new Error(await res.text() || 'Unable to start the payment')
            const { url } = await res.json() as { url: string }
            window.location.href = url
        } catch (err) {
            toast.error(err instanceof Error ? err.message : 'Unable to start the payment')
            setStarting(false)
        }
    }, [csrfToken, toast])

    return { startCheckout, starting }
}

/**
 * Reads the session id Stripe put in the return URL, and strips it so a
 * refresh can't replay it.
 *
 * Consuming the parameter on the first read is what makes this safe to call
 * from an effect that React runs twice in development: the second call sees a
 * clean URL and returns null. The engine is idempotent regardless — a session
 * already credited bounces off the ledger's unique rail_ref — but a second
 * request would still show the member a confusing error, so it is better not
 * to make it.
 */
export function stripeCheckoutResult(): { sessionId: string } | { cancelled: true } | null {
    const params = new URLSearchParams(window.location.search)
    const flag = params.get('stripe')
    if (flag !== 'success' && flag !== 'cancelled') return null

    const sessionId = params.get('session_id')
    params.delete('stripe')
    params.delete('session_id')
    const query = params.toString()
    window.history.replaceState({}, '', window.location.pathname + (query ? `?${query}` : ''))

    if (flag === 'cancelled') return { cancelled: true }
    // "success" with no session id shouldn't happen — Stripe substitutes the
    // placeholder itself — but a hand-typed URL shouldn't confirm anything.
    return sessionId ? { sessionId } : null
}
