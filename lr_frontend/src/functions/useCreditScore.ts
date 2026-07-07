import { useEffect, useState } from 'react'

const API = import.meta.env.VITE_API_URL ?? ''

export const CREDIT_SCORE_MIN = 0
export const CREDIT_SCORE_MAX = 150
export const CREDIT_SCORE_DEFAULT = 50

export type CreditScore = {
    score: number
    updatedAt: number
}

/** Read-only for now — nothing in the codebase yet computes or adjusts the
 *  score, so there's no shared context to keep in sync, just a fetch per mount. */
export function useCreditScore() {
    const [score, setScore] = useState<CreditScore | null>(null)
    const [loading, setLoading] = useState(true)
    const [error, setError] = useState(false)

    useEffect(() => {
        let aborted = false
        setLoading(true)
        setError(false)
        fetch(`${API}/credit/score`, { credentials: 'include' })
            .then(async res => {
                if (!res.ok) throw new Error()
                return res.json() as Promise<{ score: number; updated_at: number }>
            })
            .then(data => { if (!aborted) setScore({ score: data.score, updatedAt: data.updated_at }) })
            .catch(() => { if (!aborted) setError(true) })
            .finally(() => { if (!aborted) setLoading(false) })
        return () => { aborted = true }
    }, [])

    return { score, loading, error }
}
