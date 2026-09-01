import { useCallback, useEffect, useState } from 'react'
import type { CollateralRecord } from './types'

const API = import.meta.env.VITE_API_URL ?? ''

/**
 * The custody record for one XLM-collateral loan (GET /loans/{id}/collateral):
 * the position, the price it was struck at with its per-feed evidence, and
 * every on-chain movement with its transaction hash.
 *
 * Fetched lazily — only when a loan row is actually opened — and kept per loan
 * id, because a settled record never changes and a pending one only changes
 * when the borrower locks, which calls `invalidate`.
 *
 * Everything the caller reads is DERIVED from the two keyed maps rather than
 * mirrored into more state: `loading` is "asked for, nothing back yet", which
 * is exactly what it means, and there is no effect that writes state
 * synchronously and re-renders on top of itself. Pass null to fetch nothing.
 *
 * `use`-prefixed per this repo's React Compiler requirement.
 */
export default function useCollateralRecord(loanId: string | null) {
    const [records, setRecords] = useState<ReadonlyMap<string, CollateralRecord>>(new Map())
    const [failed, setFailed] = useState<ReadonlySet<string>>(new Set())

    useEffect(() => {
        if (!loanId || records.has(loanId) || failed.has(loanId)) return
        const controller = new AbortController()
        void (async () => {
            try {
                const res = await fetch(`${API}/loans/${loanId}/collateral`, {
                    credentials: 'include',
                    signal: controller.signal,
                })
                if (!res.ok) throw new Error()
                const data = await res.json() as CollateralRecord
                if (!controller.signal.aborted) {
                    setRecords(prev => new Map(prev).set(loanId, data))
                }
            } catch {
                if (!controller.signal.aborted) {
                    setFailed(prev => new Set(prev).add(loanId))
                }
            }
        })()
        return () => controller.abort()
    }, [loanId, records, failed])

    /** Forget a record so the next open re-reads it — after a lock lands. */
    const invalidate = useCallback((id: string) => {
        setRecords(prev => {
            if (!prev.has(id)) return prev
            const next = new Map(prev)
            next.delete(id)
            return next
        })
        setFailed(prev => {
            if (!prev.has(id)) return prev
            const next = new Set(prev)
            next.delete(id)
            return next
        })
    }, [])

    const record = loanId ? records.get(loanId) ?? null : null
    const error = loanId ? failed.has(loanId) : false

    return { record, error, loading: !!loanId && !record && !error, invalidate }
}
