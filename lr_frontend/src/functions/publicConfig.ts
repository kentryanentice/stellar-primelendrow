import { apiFetch } from './apiFetch'

const API = import.meta.env.VITE_API_URL ?? ''

/**
 * The public identifiers the app needs at runtime, served by the engine's
 * GET /config instead of being baked into the build (lr_engine api::config).
 * Public by design — the browser needs them to load PayPal's buttons and to
 * reach WalletConnect — so fetching them hides nothing; it just keeps one
 * place to set them.
 */
export type PublicConfig = {
    paypal_client_id: string | null
    walletconnect_project_id: string | null
}

const NONE: PublicConfig = { paypal_client_id: null, walletconnect_project_id: null }

let pending: Promise<PublicConfig> | null = null

/** Fetched once per page load and shared by every caller. A failed fetch
 *  resolves to "nothing configured" — the same state a missing value always
 *  meant — and is retried by the next caller rather than cached. */
export function loadPublicConfig(): Promise<PublicConfig> {
    pending ??= (async () => {
        try {
            const res = await apiFetch(`${API}/config`, { credentials: 'include' })
            if (!res.ok) throw new Error()
            const data = await res.json() as Partial<PublicConfig>
            return {
                paypal_client_id: data.paypal_client_id ?? null,
                walletconnect_project_id: data.walletconnect_project_id ?? null,
            }
        } catch {
            pending = null
            return NONE
        }
    })()
    return pending
}
