import { useEffect, useState } from 'react'
import { useSession } from '../providers/useSession'

const API = import.meta.env.VITE_API_URL ?? ''

export const CREDIT_SCORE_MIN = 0
export const CREDIT_SCORE_MAX = 150
export const CREDIT_SCORE_DEFAULT = 50

export type CreditScore = {
    score: number
    updatedAt: number
}

/** Read-only for now — nothing in the codebase yet computes or adjusts the
 *  score, so there's no shared context to keep in sync, just a fetch per mount.
 *  Not fetched for Pending/Verifying accounts: they're still gated out of
 *  member features, so a score is premature — every caller (Settings, Sidebar)
 *  gets this for free instead of each having to remember to check the role. */
export function useCreditScore() {
    const { user } = useSession()
    const eligible = user?.role !== 'Pending' && user?.role !== 'Verifying'

    const [score, setScore] = useState<CreditScore | null>(null)
    const [loading, setLoading] = useState(eligible)
    const [error, setError] = useState(false)

    useEffect(() => {
        if (!eligible) {
            setScore(null)
            setLoading(false)
            setError(false)
            return
        }
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
    }, [eligible])

    return { score, loading, error }
}
