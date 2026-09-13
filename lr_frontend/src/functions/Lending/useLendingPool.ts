import { useCallback, useEffect, useState } from 'react'
import type { PoolResponse } from './types'
import { apiFetch } from '../apiFetch'

const API = import.meta.env.VITE_API_URL ?? ''

/** GET /pool — reads only; `refresh` and the first-load effect decide what to do with it. */
async function fetchPool(signal?: AbortSignal) {
    const res = await apiFetch(`${API}/pool`, { credentials: 'include', signal })
    if (!res.ok) throw new Error()
    return await res.json() as PoolResponse
}

/**
 * The one read that drives the whole Lending page: pool stats, the caller's
 * own funds (four numbers + lots), and the engine's parameters (policy
 * bands, fx rate, contract id). Everything the page displays comes from
 * here — the UI never derives a rule locally. `use`-prefixed per this repo's
 * React Compiler requirement.
 */
export default function useLendingPool() {
    const [data, setData] = useState<PoolResponse | null>(null)
    const [loading, setLoading] = useState(true)
    const [error, setError] = useState(false)

    const refresh = useCallback(async () => {
        setError(false)
        try {
            setData(await fetchPool())
        } catch {
            setError(true)
        } finally {
            setLoading(false)
        }
    }, [])

    // First load. The initial state already says "loading", so state is only
    // written once the response is in — never synchronously in the effect.
    useEffect(() => {
        const controller = new AbortController()
        void (async () => {
            try {
                const pool = await fetchPool(controller.signal)
                if (!controller.signal.aborted) setData(pool)
            } catch {
                if (!controller.signal.aborted) setError(true)
            } finally {
                if (!controller.signal.aborted) setLoading(false)
            }
        })()
        return () => controller.abort()
    }, [])

    return { data, loading, error, refresh }
}
