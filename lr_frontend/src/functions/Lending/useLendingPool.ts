import { useCallback, useEffect, useState } from 'react'
import type { PoolResponse } from './types'

const API = import.meta.env.VITE_API_URL ?? ''

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
            const res = await fetch(`${API}/pool`, { credentials: 'include' })
            if (!res.ok) throw new Error()
            setData(await res.json() as PoolResponse)
        } catch {
            setError(true)
        } finally {
            setLoading(false)
        }
    }, [])

    useEffect(() => {
        void refresh()
    }, [refresh])

    return { data, loading, error, refresh }
}
