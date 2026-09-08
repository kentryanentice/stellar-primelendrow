import { useCallback } from 'react'
import { useSession } from '../../providers/useSession'

const API = import.meta.env.VITE_API_URL ?? ''

/** What a PayPal payment is for. Mirrors the engine's whitelist. */
export type OrderPurpose = 'deposit' | 'repay'

/**
 * Asks the engine to create the PayPal order the member will approve.
 *
 * The Buttons SDK can build an order in the browser, and this app used to let
 * it. Two things were wrong with that: the amount came from the page, and
 * nothing on the order recorded whose payment it was — so an order id, which
 * is a bearer reference, could be presented by anyone and credited to them.
 *
 * The engine now creates it and stamps the caller's id into `custom_id`, which
 * it checks again at capture. This hook exists only to fetch the resulting id;
 * nothing about the payment is decided here, which is the same shape the
 * Stripe rail has always had.
 *
 * `use`-prefixed per this repo's React Compiler requirement.
 */
export default function usePayPalOrder() {
    const { csrfToken } = useSession()

    /** Resolves to PayPal's order id, or throws so the SDK cancels the flow. */
    const createOrder = useCallback(async (
        centavos: number,
        purpose: OrderPurpose = 'deposit',
        loanId?: string,
    ): Promise<string> => {
        const res = await fetch(`${API}/paypal/order`, {
            method: 'POST',
            credentials: 'include',
            headers: {
                'Content-Type': 'application/json',
                ...(csrfToken ? { 'x-csrf-token': csrfToken } : {}),
            },
            body: JSON.stringify({
                amount: centavos,
                purpose,
                ...(loanId ? { loan_id: loanId } : {}),
            }),
        })
        if (!res.ok) throw new Error(await res.text() || 'Unable to start the payment')
        const { order_id } = await res.json() as { order_id: string }
        return order_id
    }, [csrfToken])

    return { createOrder }
}
