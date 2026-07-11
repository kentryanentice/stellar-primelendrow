import { useCallback, useEffect, useState } from 'react'
import { useSession } from '../../providers/useSession'
import { useToast } from '../../providers/useToast'
import { ID_TYPE_LABELS, type IdType } from '../KYC/idParsing'

const API = import.meta.env.VITE_API_URL ?? ''

/** Below this, a face-match score gets a "low match" flag; at/above 85 it reads
 *  as a strong match. Matches how the queue cards color-code themselves. */
const VERDICT_LOW_MAX = 60
const VERDICT_STRONG_MIN = 85

export const REJECT_PRESETS = [
    'Face mismatch',
    'Blurred or illegible ID',
    'Expired document',
    'Data mismatch',
    'Suspected fraud',
    'Duplicate application',
]

export type PendingItem = {
    id: string
    user_id: string
    id_type: string
    face_match_score: number | null
    liveness_passed: boolean
    created_at: number
    /** Signed, short-lived (~5 min) — a stale link just means refreshing the queue for fresh ones. */
    id_image_url: string | null
    selfie_image_url: string | null
}

type PendingResponse = {
    items: PendingItem[]
    total: number
    page: number
    page_size: number
    total_pages: number
}

export type SubmissionDetail = {
    id: string
    user_id: string
    status: string
    id_type: string
    first_name: string
    middle_name: string | null
    last_name: string
    dob: string
    id_number: string
    wallet_address: string | null
    face_match_score: number | null
    liveness_passed: boolean
    /** Signed, short-lived (~5 min) — a stale link just means re-opening the submission for a fresh one. */
    id_image_url: string | null
    selfie_image_url: string | null
    rejection_reason: string | null
    created_at: number
    reviewed_at: number | null
}

export const idTypeLabel = (idType: string) => ID_TYPE_LABELS[idType as IdType] ?? idType

// created_at/reviewed_at are unix seconds (Utc::now().timestamp() server-side)
export const formatDate = (secs: number) =>
    new Date(secs * 1000).toLocaleString(undefined, {
        year: 'numeric', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit',
    })

export const timeAgo = (secs: number) => {
    const minutes = Math.max(0, Math.floor((Date.now() - secs * 1000) / 60_000))
    if (minutes < 1) return 'just now'
    if (minutes < 60) return `${minutes}m ago`
    if (minutes < 1440) return `${Math.floor(minutes / 60)}h ago`
    return `${Math.floor(minutes / 1440)}d ago`
}

export type Verdict = { label: string; cls: 'is-strong' | 'is-review' | 'is-low' }

/** No score at all (bypassed comparison) reads the same as a low match — there's
 *  nothing to vouch for it either way. */
export const scoreVerdict = (score: number | null): Verdict => {
    const pct = score ?? 0
    if (pct >= VERDICT_STRONG_MIN) return { label: 'Strong match', cls: 'is-strong' }
    if (pct >= VERDICT_LOW_MAX) return { label: 'Needs review', cls: 'is-review' }
    return { label: 'Low match', cls: 'is-low' }
}
export const isFlagged = (score: number | null) => (score ?? 0) < VERDICT_LOW_MAX

/**
 * Drives the admin KYC review queue: a paginated list of pending submissions
 * (including signed thumbnail URLs, since the queue's own cards show them),
 * on-demand decrypted detail for the one submission actually opened (audited
 * server-side, never preloaded beyond the thumbnails), and the approve/reject
 * decision. Mirrors useKYCFunctions — the page only renders what this returns.
 */
export default function useAdminFunctions() {
    const { csrfToken } = useSession()
    const toast = useToast()

    // ---- queue (paginated) ----
    const [queue, setQueue] = useState<PendingItem[]>([])
    const [page, setPage] = useState(1)
    const [total, setTotal] = useState(0)
    const [totalPages, setTotalPages] = useState(1)
    const [queueLoading, setQueueLoading] = useState(true)
    const [queueLoadingMore, setQueueLoadingMore] = useState(false)
    const [queueError, setQueueError] = useState(false)

    // ---- mode: browsing the carousel, or reviewing one submission ----
    const [mode, setMode] = useState<'browse' | 'review'>('browse')
    const [selectedId, setSelectedId] = useState<string | null>(null)
    const [detail, setDetail] = useState<SubmissionDetail | null>(null)
    const [detailLoading, setDetailLoading] = useState(false)
    const [detailError, setDetailError] = useState(false)

    // ---- full-size image lightbox (manual selfie-vs-ID comparison) ----
    const [lightboxOpen, setLightboxOpen] = useState(false)

    // ---- decision ----
    const [rejecting, setRejecting] = useState(false)
    const [reason, setReason] = useState('')
    const [deciding, setDeciding] = useState(false)

    // page_size is fixed server-side (10) — this only ever tells the backend
    // which page, never how large, so there's no client-controlled query
    // string; page travels in the POST body instead
    const fetchQueuePage = useCallback(async (targetPage: number): Promise<PendingResponse | null> => {
        const res = await fetch(`${API}/kyc/admin/pending`, {
            method: 'POST',
            credentials: 'include',
            headers: {
                'Content-Type': 'application/json',
                ...(csrfToken ? { 'x-csrf-token': csrfToken } : {}),
            },
            body: JSON.stringify({ page: targetPage }),
        })
        if (!res.ok) return null
        return res.json() as Promise<PendingResponse>
    }, [csrfToken])

    const loadQueue = useCallback(async (targetPage: number) => {
        setQueueLoading(true)
        setQueueError(false)
        try {
            let data = await fetchQueuePage(targetPage)
            if (!data) throw new Error()
            // the page we asked for might no longer exist (e.g. we just decided
            // the only submission on what was the last page) — fall back to
            // whatever the new last page is instead of showing an empty page 3 of 2
            if (data.items.length === 0 && data.page > 1 && data.page > data.total_pages) {
                data = await fetchQueuePage(data.total_pages)
                if (!data) throw new Error()
            }
            setQueue(data.items)
            setPage(data.page)
            setTotal(data.total)
            setTotalPages(data.total_pages)
        } catch {
            setQueueError(true)
        } finally {
            setQueueLoading(false)
        }
    }, [fetchQueuePage])

    useEffect(() => { loadQueue(1) }, [loadQueue])

    // appends the next backend batch onto the already-loaded queue instead of
    // replacing it, so scrolling the carousel to the end feels continuous
    // rather than resetting back to a fresh 3-card view
    const loadMore = useCallback(async () => {
        if (queueLoadingMore || queueLoading || page >= totalPages) return
        setQueueLoadingMore(true)
        try {
            const data = await fetchQueuePage(page + 1)
            if (!data) return
            setQueue(prev => [...prev, ...data.items])
            setPage(data.page)
            setTotal(data.total)
            setTotalPages(data.total_pages)
        } finally {
            setQueueLoadingMore(false)
        }
    }, [queueLoadingMore, queueLoading, page, totalPages, fetchQueuePage])

    const closeReview = useCallback(() => {
        setMode('browse')
        setSelectedId(null)
        setDetail(null)
        setRejecting(false)
        setReason('')
        setLightboxOpen(false)
    }, [])

    // decrypts PII + mints signed image URLs and is audited server-side — only
    // fetched for the one submission an admin deliberately opens
    const openReview = useCallback(async (id: string) => {
        setMode('review')
        setSelectedId(id)
        setDetail(null)
        setDetailError(false)
        setRejecting(false)
        setReason('')
        setLightboxOpen(false)
        setDetailLoading(true)
        try {
            const res = await fetch(`${API}/kyc/admin/submissions/${id}`, { credentials: 'include' })
            if (!res.ok) throw new Error()
            setDetail(await res.json())
        } catch {
            setDetailError(true)
        } finally {
            setDetailLoading(false)
        }
    }, [])

    const openLightbox = useCallback(() => setLightboxOpen(true), [])
    const closeLightbox = useCallback(() => setLightboxOpen(false), [])

    // Escape closes whichever layer is on top: the lightbox first, then review mode
    useEffect(() => {
        if (mode !== 'review') return
        const onKey = (e: KeyboardEvent) => {
            if (e.key !== 'Escape') return
            if (lightboxOpen) closeLightbox()
            else closeReview()
        }
        window.addEventListener('keydown', onKey)
        return () => window.removeEventListener('keydown', onKey)
    }, [mode, lightboxOpen, closeLightbox, closeReview])

    const decide = useCallback(async (decision: 'approve' | 'reject') => {
        if (!selectedId || deciding) return
        if (decision === 'reject' && !reason.trim()) {
            toast.error('A rejection needs a reason')
            return
        }
        setDeciding(true)
        try {
            const res = await fetch(`${API}/kyc/admin/review`, {
                method: 'POST',
                credentials: 'include',
                headers: {
                    'Content-Type': 'application/json',
                    ...(csrfToken ? { 'x-csrf-token': csrfToken } : {}),
                },
                body: JSON.stringify({
                    submission_id: selectedId,
                    decision,
                    ...(decision === 'reject' ? { reason: reason.trim() } : {}),
                }),
            })
            // a 404 here means another admin already decided this one first —
            // the status guard in review() makes that race safe, so just
            // reconcile the queue instead of treating it as a failure
            const raced = res.status === 404
            if (!raced && !res.ok) throw new Error(await res.text() || 'Review failed')

            if (raced) toast.info('Someone else already reviewed this submission')
            else toast.success(decision === 'approve' ? 'Submission approved' : 'Submission rejected')

            // auto-advance to the next item in the (locally known) queue while in
            // review mode, so deciding one submission doesn't force a trip back
            // to the carousel — loadQueue below reconciles this against the server
            const idx = queue.findIndex(item => item.id === selectedId)
            const remaining = queue.filter(item => item.id !== selectedId)
            if (mode === 'review' && remaining.length > 0) {
                const next = remaining[Math.min(idx, remaining.length - 1)]
                openReview(next.id)
            } else {
                closeReview()
            }
            loadQueue(page)
        } catch (err) {
            toast.error(err instanceof Error ? err.message : 'Review failed')
        } finally {
            setDeciding(false)
        }
    }, [selectedId, deciding, reason, csrfToken, toast, mode, queue, openReview, closeReview, loadQueue, page])

    return {
        // queue
        queue, page, total, totalPages, queueLoading, queueLoadingMore, queueError, loadQueue, loadMore,

        // mode / selected submission
        mode, selectedId, detail, detailLoading, detailError, openReview, closeReview,

        // full-size image lightbox
        lightboxOpen, openLightbox, closeLightbox,

        // decision
        rejecting, setRejecting, reason, setReason, deciding, decide,
    }
}
