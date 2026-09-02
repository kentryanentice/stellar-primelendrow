import { useCallback, useEffect, useState } from 'react'
import { useSession } from '../../providers/useSession'
import { useToast } from '../../providers/useToast'
import type { PaypalAccount } from './types'

const API = import.meta.env.VITE_API_URL ?? ''

/**
 * The member's connected PayPal — the destination loan proceeds are paid to.
 *
 * Connecting is a full-page navigation to PayPal, not a popup or an iframe:
 * PayPal refuses to be framed, and a redirect is also what makes the login
 * happen on paypal.com where it belongs. The engine mints a single-use state
 * token, PayPal sends the member back to the engine's callback, and the
 * callback redirects here with `?paypal=connected` (or `?paypal=error`).
 *
 * `use`-prefixed per this repo's React Compiler requirement.
 */
export default function usePaypalAccount() {
    const { csrfToken } = useSession()
    const toast = useToast()

    const [account, setAccount] = useState<PaypalAccount | null>(null)
    const [loading, setLoading] = useState(true)
    const [busy, setBusy] = useState(false)

    const refresh = useCallback(async () => {
        try {
            const res = await fetch(`${API}/paypal/account`, { credentials: 'include' })
            if (!res.ok) throw new Error()
            setAccount(await res.json() as PaypalAccount)
        } catch {
            setAccount(null)
        } finally {
            setLoading(false)
        }
    }, [])

    useEffect(() => { void refresh() }, [refresh])

    /** Ask the engine where to send them, then leave the app. */
    const connect = useCallback(async () => {
        setBusy(true)
        try {
            const res = await fetch(`${API}/paypal/connect`, { credentials: 'include' })
            if (!res.ok) throw new Error(await res.text() || 'Unable to start the PayPal connection')
            const { url } = await res.json() as { url: string }
            window.location.href = url
        } catch (err) {
            toast.error(err instanceof Error ? err.message : 'Unable to start the PayPal connection')
            setBusy(false)
        }
    }, [toast])

    const disconnect = useCallback(async () => {
        setBusy(true)
        try {
            const res = await fetch(`${API}/paypal/disconnect`, {
                method: 'POST',
                credentials: 'include',
                headers: {
                    'Content-Type': 'application/json',
                    ...(csrfToken ? { 'x-csrf-token': csrfToken } : {}),
                },
            })
            if (!res.ok) throw new Error(await res.text() || 'Unable to disconnect PayPal')
            const { message } = await res.json() as { message: string }
            toast.success(message)
            await refresh()
        } catch (err) {
            toast.error(err instanceof Error ? err.message : 'Unable to disconnect PayPal')
        } finally {
            setBusy(false)
        }
    }, [csrfToken, refresh, toast])

    return { account, loading, busy, connect, disconnect, refresh }
}

/**
 * Reads the `?paypal=` flag the engine's callback redirects back with, tells
 * the member how it went, and strips it from the URL so a refresh doesn't
 * repeat the message.
 */
export function paypalRedirectResult(): { ok: boolean; message: string } | null {
    const params = new URLSearchParams(window.location.search)
    const flag = params.get('paypal')
    if (!flag) return null

    params.delete('paypal')
    const reason = params.get('reason')
    params.delete('reason')
    const query = params.toString()
    window.history.replaceState({}, '', window.location.pathname + (query ? `?${query}` : ''))

    if (flag === 'connected') {
        return { ok: true, message: 'PayPal connected — loan proceeds will be sent there' }
    }
    return { ok: false, message: REASONS[reason ?? ''] ?? 'PayPal couldn’t be connected. Please try again.' }
}

/** The engine's callback only ever sends these short codes — never a raw error. */
const REASONS: Record<string, string> = {
    declined: 'You cancelled the PayPal connection.',
    expired: 'That took too long — start the connection again.',
    badstate: 'That connection link was already used. Start again.',
    taken: 'That PayPal account is already linked to another member.',
    paypal: 'PayPal couldn’t confirm the account. Please try again.',
    nocode: 'PayPal didn’t complete the connection.',
    nostate: 'PayPal didn’t complete the connection.',
    server: 'Something went wrong on our side. Please try again.',
}
