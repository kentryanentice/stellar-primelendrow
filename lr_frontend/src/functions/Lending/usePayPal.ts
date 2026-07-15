import { useEffect, useState } from 'react'

/**
 * Loads the PayPal JS SDK once (public client id only — the secret lives in
 * lr_engine, which is what actually captures and verifies every order).
 * Exposes window.paypal when ready.
 */

const CLIENT_ID = import.meta.env.VITE_PAYPAL_CLIENT_ID as string | undefined

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
        style?: { layout?: string; color?: string; shape?: string; height?: number; label?: string }
        createOrder: (data: unknown, actions: PayPalOrderActions) => Promise<string>
        onApprove: (data: { orderID: string }) => Promise<void>
        onError?: (err: unknown) => void
        onCancel?: () => void
    }) => PayPalButtonsInstance
}

declare global {
    interface Window {
        paypal?: PayPalNamespace
    }
}

let scriptPromise: Promise<PayPalNamespace | null> | null = null

function loadSdk(): Promise<PayPalNamespace | null> {
    if (!CLIENT_ID) return Promise.resolve(null)
    if (window.paypal) return Promise.resolve(window.paypal)
    if (!scriptPromise) {
        scriptPromise = new Promise(resolve => {
            const script = document.createElement('script')
            // currency is pinned to PHP — the engine refuses any other
            // currency at capture time regardless of what a tampered page asks
            script.src = `https://www.paypal.com/sdk/js?client-id=${encodeURIComponent(CLIENT_ID)}&currency=PHP&intent=capture&components=buttons`
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

    useEffect(() => {
        let aborted = false
        void loadSdk().then(ns => {
            if (aborted) return
            if (ns) setPaypal(ns)
            else setFailed(true)
        })
        return () => { aborted = true }
    }, [])

    return { paypal, failed, configured: Boolean(CLIENT_ID) }
}
