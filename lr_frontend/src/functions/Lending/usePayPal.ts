import { useEffect, useState } from 'react'
import { loadPublicConfig } from '../publicConfig'

/**
 * Loads the PayPal JS SDK once (public client id only — the secret lives in
 * lr_engine, which is what actually captures and verifies every order).
 * Exposes window.paypal when ready. The client id comes from the engine's
 * GET /config, so it is always the one the engine itself uses with PayPal.
 */

export type PayPalOrderActions = {
    order: {
        create: (options: {
            intent: 'CAPTURE'
            purchase_units: { amount: { currency_code: 'PHP'; value: string }; description?: string }[]
        }) => Promise<string>
    }
}

export type PayPalButtonsInstance = {
    render: (container: HTMLElement) => Promise<void>
    close: () => Promise<void>
}

export type PayPalNamespace = {
    Buttons: (config: {
        style?: { layout?: string; color?: string; shape?: string; height?: number; label?: string; tagline?: boolean }
        createOrder: (data: unknown, actions: PayPalOrderActions) => Promise<string>
        onApprove: (data: { orderID: string }) => Promise<void>
        onError?: (err: unknown) => void
        /** The member closed PayPal's window. Carries the order it abandoned. */
        onCancel?: (data: { orderID?: string }) => void
    }) => PayPalButtonsInstance
}

declare global {
    interface Window {
        paypal?: PayPalNamespace
    }
}

let scriptPromise: Promise<PayPalNamespace | null> | null = null

function loadSdk(clientId: string): Promise<PayPalNamespace | null> {
    if (window.paypal) return Promise.resolve(window.paypal)
    if (!scriptPromise) {
        scriptPromise = new Promise(resolve => {
            const script = document.createElement('script')
            // currency is pinned to PHP — the engine refuses any other
            // currency at capture time regardless of what a tampered page asks
            script.src = `https://www.paypal.com/sdk/js?client-id=${encodeURIComponent(clientId)}&currency=PHP&intent=capture&components=buttons`
            script.async = true
            script.onload = () => resolve(window.paypal ?? null)
            script.onerror = () => {
                scriptPromise = null // allow a retry on the next mount
                resolve(null)
            }
            document.head.appendChild(script)
        })
    }
    return scriptPromise
}

/** Resolves the PayPal namespace once the SDK script is on the page.
 *  `use`-prefixed per this repo's React Compiler requirement. */
export default function usePayPal() {
    const [paypal, setPaypal] = useState<PayPalNamespace | null>(null)
    const [failed, setFailed] = useState(false)
    // Assumed until the engine says otherwise, so loading looks exactly as it
    // always did (an empty button area) rather than flashing "not configured".
    const [configured, setConfigured] = useState(true)

    useEffect(() => {
        let aborted = false
        void (async () => {
            const { paypal_client_id: clientId } = await loadPublicConfig()
            if (aborted) return
            if (!clientId) {
                setConfigured(false)
                return
            }
            const ns = await loadSdk(clientId)
            if (aborted) return
            if (ns) setPaypal(ns)
            else setFailed(true)
        })()
        return () => { aborted = true }
    }, [])

    return { paypal, failed, configured }
}
