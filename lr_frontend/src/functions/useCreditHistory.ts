import { useEffect, useState } from 'react'
import { apiFetch } from './apiFetch'

const API = import.meta.env.VITE_API_URL ?? ''

/** A change that happened, newest first. `score_from` is null only on the
 *  account's opening row; `note` is the logged sentence, sent only for rows
 *  that carry no reason code. */
export type ScoreChange = {
    score_from: number | null
    score_to: number
    reason: string | null
    note: string | null
    loan_id: string | null
    at: number
}

/** A rise already earned by paying a loan off, waiting on its term to end.
 *  From and to assume nothing else lands first. */
export type UpcomingRise = {
    loan_id: string
    due_at: number
    paid_off_at: number | null
    term_end: number | null
    score_from: number
    score_to: number
}

/** One way an open loan can still end, from today's score alone. */
export type ScoreOutcome = {
    reason: string
    delta: number
    score_to: number
}

export type ScoreAtStake = {
    loan_id: string
    role: 'borrower' | 'guarantor'
    status: 'active' | 'defaulted' | 'reconciling'
    term_end: number | null
    /** The oldest unpaid installment past its due date. */
    overdue_since: number | null
    /** Accepted pledges behind the member's own loan; 0 on a guarantor's row. */
    guarantors: number
    outcomes: ScoreOutcome[]
}

export type CreditHistory = {
    score: number
    changes: ScoreChange[]
    upcoming: UpcomingRise[]
    at_stake: ScoreAtStake[]
}

/** GET /credit/history — the member's own score: what moved it, what is due
 *  to, and what each open loan or pledge still could. Every number, the
 *  projections included, comes from the engine's own scoring rules. */
export function useCreditHistory() {
    const [data, setData] = useState<CreditHistory | null>(null)
    const [loading, setLoading] = useState(true)
    const [error, setError] = useState(false)

    // The initial state already says "loading", so state is only written once
    // the response is in — never synchronously in the effect.
    useEffect(() => {
        const controller = new AbortController()
        void (async () => {
            try {
                const res = await apiFetch(`${API}/credit/history`, { credentials: 'include', signal: controller.signal })
                if (!res.ok) throw new Error()
                const history = await res.json() as CreditHistory
                if (!controller.signal.aborted) setData(history)
            } catch {
                if (!controller.signal.aborted) setError(true)
            } finally {
                if (!controller.signal.aborted) setLoading(false)
            }
        })()
        return () => controller.abort()
    }, [])

    return { data, loading, error }
}
