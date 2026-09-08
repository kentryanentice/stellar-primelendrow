import { useCallback, useEffect, useState } from 'react'
import { useSession } from '../../providers/useSession'
import { useToast } from '../../providers/useToast'
import type { StripeAccount } from './types'

const API = import.meta.env.VITE_API_URL ?? ''

/**
 * The member's connected Stripe account — the destination loan proceeds and
 * withdrawals are paid to when this deployment runs the Stripe rail.
 *
 * The mirror of `usePaypalAccount`, with one difference worth knowing about.
 * PayPal linking is instant: the member signs in, and the account is payable.
 * Stripe onboarding is a form — identity details, a bank account — that they
 * can abandon half-way, and that Stripe may take time to clear afterwards. So
 * there are three states here, not two: not started, started but not payable
 * (`onboarding`), and payable (`connected`). The card must not call the middle
 * one "connected", or a member finds out at withdrawal.
 *
 * Connecting is a full-page navigation to Stripe, not a popup or an iframe:
 * Stripe refuses to be framed, and a redirect is also what makes the identity
 * step happen on stripe.com where it belongs. The engine mints a single-use
 * state token, Stripe sends the member back to the engine's return URL, and
 * that redirects here with `?stripe=connected` (or `?stripe=error`).
 *
 * `use`-prefixed per this repo's React Compiler requirement.
 */
export default function useStripeAccount() {
    const { csrfToken } = useSession()
    const toast = useToast()

    const [account, setAccount] = useState<StripeAccount | null>(null)
    const [loading, setLoading] = useState(true)
    const [busy, setBusy] = useState(false)

    const refresh = useCallback(async () => {
        try {
            const res = await fetch(`${API}/stripe/account`, { credentials: 'include' })
            if (!res.ok) throw new Error()
            setAccount(await res.json() as StripeAccount)
        } catch {
            setAccount(null)
        } finally {
            setLoading(false)
        }
    }, [])

    useEffect(() => { void refresh() }, [refresh])

    /**
     * Ask the engine where to send them, then leave the app. The same call
     * starts onboarding and resumes an abandoned one — the engine reuses the
     * member's existing connected account rather than creating a second.
     */
    const connect = useCallback(async () => {
        setBusy(true)
        try {
            const res = await fetch(`${API}/stripe/connect`, { credentials: 'include' })
            if (!res.ok) throw new Error(await res.text() || 'Unable to start the Stripe connection')
            const { url } = await res.json() as { url: string }
            window.location.href = url
        } catch (err) {
            toast.error(err instanceof Error ? err.message : 'Unable to start the Stripe connection')
            setBusy(false)
        }
    }, [toast])

    const disconnect = useCallback(async () => {
        setBusy(true)
        try {
            const res = await fetch(`${API}/stripe/disconnect`, {
                method: 'POST',
                credentials: 'include',
                headers: {
                    'Content-Type': 'application/json',
                    ...(csrfToken ? { 'x-csrf-token': csrfToken } : {}),
                },
            })
            if (!res.ok) throw new Error(await res.text() || 'Unable to disconnect Stripe')
            const { message } = await res.json() as { message: string }
            toast.success(message)
            await refresh()
        } catch (err) {
            toast.error(err instanceof Error ? err.message : 'Unable to disconnect Stripe')
        } finally {
            setBusy(false)
        }
    }, [csrfToken, refresh, toast])

    return { account, loading, busy, connect, disconnect, refresh }
}

/**
 * Reads the `?stripe=` flag the engine's return URL redirects back with, tells
 * the member how it went, and strips it from the URL so a refresh doesn't
 * repeat the message.
 *
 * Deliberately ignores `?stripe=success`, which is the *checkout* return and
 * belongs to `stripeCheckoutResult` — the two flows share a query key because
 * they share a provider, and each reads only its own values.
 */
export function stripeConnectResult(): { ok: boolean; message: string } | null {
    const params = new URLSearchParams(window.location.search)
    const flag = params.get('stripe')
    if (flag !== 'connected' && flag !== 'error') return null

    params.delete('stripe')
    const reason = params.get('reason')
    params.delete('reason')
    const query = params.toString()
    window.history.replaceState({}, '', window.location.pathname + (query ? `?${query}` : ''))

    if (flag === 'connected') {
        return { ok: true, message: 'Stripe connected — payouts will be sent there' }
    }
    return { ok: false, message: REASONS[reason ?? ''] ?? 'Stripe couldn’t be connected. Please try again.' }
}

/** The engine's callback only ever sends these short codes — never a raw error. */
const REASONS: Record<string, string> = {
    // Not a failure so much as an unfinished job — the wording says what to do.
    incomplete: 'Stripe still needs more information before it can pay you. Continue where you left off.',
    expired: 'That setup link expired — start the connection again.',
    badstate: 'That setup link was already used. Start again.',
    nostate: 'Stripe didn’t complete the connection.',
    noaccount: 'We couldn’t find your Stripe account. Start the connection again.',
    stripe: 'Stripe couldn’t confirm the account. Please try again.',
    server: 'Something went wrong on our side. Please try again.',
}
