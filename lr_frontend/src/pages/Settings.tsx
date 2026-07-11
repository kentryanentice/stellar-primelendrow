import { lazy, Suspense, useCallback, useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import {
    ShieldCheck, ShieldQuestion, LogOut, Bubbles, Leaf,
    BadgeCheck, CircleAlert, Gauge, Hourglass, Check, Clock, X, type LucideIcon,
} from 'lucide-react'

import { useSession } from '../providers/useSession'
import { useToast } from '../providers/useToast'
import { useAccent } from '../providers/AccentProvider'
import { useCreditScore, CREDIT_SCORE_MAX } from '../functions/useCreditScore'

// The wallet-connect SDKs (Freighter/WalletConnect) this pulls in shouldn't
// add weight to every Settings visit — same rationale as the KYC page's own
// lazy-load in RoutesProvider.
const WalletsCard = lazy(() => import('../elements/Settings/WalletsCard'))

const API = import.meta.env.VITE_API_URL ?? ''
const LOGO = '/pictures/lr.png'
/** Keeps the sign-out overlay on screen at least this long, so a fast response doesn't just flash. */
const LOGOUT_MIN_DISPLAY_MS = 550

type KycStatusValue = 'none' | 'verifying' | 'approved' | 'rejected'
type KycStatus = {
    status: KycStatusValue
    submitted_at: number | null
    reviewed_at: number | null
    rejection_reason: string | null
}

const initialsOf = (name: string) => {
    const parts = name.trim().split(/\s+/).filter(Boolean)
    if (!parts.length) return '?'
    const first = parts[0][0]
    const last = parts.length > 1 ? parts[parts.length - 1][0] : parts[0][1]
    return `${first}${last ?? ''}`.toUpperCase()
}

/** 'User' is a KYC-passed member — surface it as "Verified" like the sidebar pip does. */
const ROLE_LABEL: Record<string, string> = { Admin: 'Admin', User: 'Verified', Pending: 'Pending', Verifying: 'Verifying' }

/** Same mapping as the sidebar's avatar pip — reused here so the hero avatar's
 *  corner badge means the same thing in both places. */
const AVATAR_BADGE: Record<string, { icon: LucideIcon; cls: string; label: string }> = {
    Admin: { icon: ShieldCheck, cls: 'is-admin', label: 'Admin' },
    User: { icon: BadgeCheck, cls: 'is-verified', label: 'Verified' },
    Pending: { icon: Hourglass, cls: 'is-pending', label: 'Pending' },
    Verifying: { icon: ShieldQuestion, cls: 'is-verifying', label: 'Verifying' },
}

/** Keyed by whatever the backend actually sends — a fallback (below) covers
 *  anything not in this map, so a stale/legacy status string (e.g. this
 *  account's row predating the "verifying" rename) never renders a blank
 *  label, just the closest honest meaning: "still under review". */
const KYC_STATUS_META: Record<string, { label: string; icon: LucideIcon; cls: string }> = {
    verifying: { label: 'Under review', icon: ShieldQuestion, cls: 'is-verifying' },
    approved: { label: 'Approved', icon: BadgeCheck, cls: 'is-approved' },
    rejected: { label: 'Rejected', icon: CircleAlert, cls: 'is-rejected' },
}
const kycStatusMeta = (status: string) => KYC_STATUS_META[status] ?? KYC_STATUS_META.verifying

// created_at/reviewed_at/submitted_at are unix seconds (Utc::now().timestamp() server-side)
const formatDate = (secs: number | null) =>
    secs == null ? '' : new Date(secs * 1000).toLocaleString(undefined, {
        year: 'numeric', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit',
    })

const formatMemberSince = (secs: number) =>
    new Date(secs * 1000).toLocaleDateString(undefined, { month: 'short', year: 'numeric' })

/** Derived from the account's own UUID — no separate backend field, and stable
 *  for the life of the account. Not a security identifier, purely cosmetic. */
const shortUserId = (id: string) => `PLR-${id.replace(/-/g, '').slice(0, 6).toUpperCase()}`

const creditTier = (score: number) =>
    score < 60 ? 'Building credit' : score < 110 ? 'Fair credit' : 'Strong credit'

type TimelineStep = {
    title: string
    time: string
    state: 'done' | 'current' | 'todo' | 'rejected'
}

/** Grounded in what /kyc/status actually returns — no fabricated timestamps.
 *  The in-progress "Under review" step has no real "last checked" time (the
 *  backend only stamps reviewed_at once a decision is made), so it just
 *  shows the label rather than inventing one. */
const verifySteps = (kyc: KycStatus): TimelineStep[] => {
    const submitted: TimelineStep = { title: 'Documents submitted', time: formatDate(kyc.submitted_at), state: 'done' }

    if (kyc.status === 'approved') {
        return [
            submitted,
            { title: 'Reviewed by our team', time: formatDate(kyc.reviewed_at), state: 'done' },
            { title: 'Approved', time: formatDate(kyc.reviewed_at), state: 'done' },
        ]
    }
    if (kyc.status === 'rejected') {
        return [
            submitted,
            { title: 'Reviewed by our team', time: formatDate(kyc.reviewed_at), state: 'done' },
            { title: 'Rejected — resubmit required', time: formatDate(kyc.reviewed_at), state: 'rejected' },
        ]
    }
    // verifying
    return [
        submitted,
        { title: 'Under review', time: 'A reviewer is checking your submission', state: 'current' },
        { title: 'Decision', time: 'Estimated within 24–48 hours', state: 'todo' },
    ]
}

type AccentKey = 'blue' | 'green'
const ACCENT_OPTIONS: { key: AccentKey; label: string; icon: LucideIcon }[] = [
    { key: 'blue', label: 'Ocean', icon: Bubbles },
    { key: 'green', label: 'Forest', icon: Leaf },
]

function Settings() {
    const { user, setUser, csrfToken } = useSession()
    const { accent, setAccent } = useAccent()
    const { score: creditScore, loading: creditLoading, error: creditError } = useCreditScore()
    const toast = useToast()
    const navigate = useNavigate()

    const [kyc, setKyc] = useState<KycStatus | null>(null)
    const [kycLoading, setKycLoading] = useState(true)
    const [kycError, setKycError] = useState(false)

    const [confirming, setConfirming] = useState(false)
    const [loggingOut, setLoggingOut] = useState(false)

    // the owner's own KYC status — no PII, just where the submission stands
    useEffect(() => {
        let aborted = false
        setKycLoading(true)
        setKycError(false)
        fetch(`${API}/kyc/status`, { credentials: 'include' })
            .then(async res => {
                if (!res.ok) throw new Error()
                return res.json() as Promise<KycStatus>
            })
            .then(data => { if (!aborted) setKyc(data) })
            .catch(() => { if (!aborted) setKycError(true) })
            .finally(() => { if (!aborted) setKycLoading(false) })
        return () => { aborted = true }
    }, [])

    // lock the page behind the sign-out confirm so it can't be scrolled or clicked
    useEffect(() => {
        if (!confirming) return
        const prev = document.body.style.overflow
        document.body.style.overflow = 'hidden'
        return () => { document.body.style.overflow = prev }
    }, [confirming])

    const performLogout = useCallback(async () => {
        setConfirming(false)
        setLoggingOut(true)
        try {
            const minDisplay = new Promise(resolve => window.setTimeout(resolve, LOGOUT_MIN_DISPLAY_MS))
            const [res] = await Promise.all([
                fetch(`${API}/auth/logout`, {
                    method: 'POST',
                    credentials: 'include',
                    headers: csrfToken ? { 'x-csrf-token': csrfToken } : undefined,
                }),
                minDisplay,
            ])
            if (!res.ok) throw new Error((await res.text()) || 'Unable to log out')
            // AccessProvider redirects to /auth as soon as user is cleared
            setUser(null)
        } catch (err) {
            setLoggingOut(false)
            toast.error(err instanceof Error ? err.message : 'Unable to log out')
        }
    }, [csrfToken, setUser, toast])

    const handleSetAccent = useCallback((key: AccentKey) => {
        if (key === accent) return
        setAccent(key)
    }, [accent, setAccent])

    const roleLabel = user ? (ROLE_LABEL[user.role] ?? user.role) : ''
    const avatarBadge = user ? AVATAR_BADGE[user.role] : undefined
    const scoreEligible = user?.role !== 'Pending' && user?.role !== 'Verifying'

    return (
        <main className='settings-page'>
            <header className='settings-head'>
                <h1>Settings</h1>
                <p>Manage your profile, verification, and appearance.</p>
            </header>

            {/* ---- profile hero ---- */}
            <section className='settings-hero'>
                <div className='settings-hero-avatar'>
                    {initialsOf(user?.username ?? '')}
                    {avatarBadge && (
                        <span className={`settings-hero-avatar-badge ${avatarBadge.cls}`} title={avatarBadge.label} aria-label={avatarBadge.label}>
                            <avatarBadge.icon aria-hidden='true' />
                        </span>
                    )}
                </div>
                <div className='settings-hero-body'>
                    <div className='settings-hero-name-row'>
                        <h2>{user?.username}</h2>
                        <span className={`settings-role-chip is-${(user?.role ?? '').toLowerCase()}`}>{roleLabel}</span>
                        {kyc && kyc.status !== 'none' && (() => {
                            const meta = kycStatusMeta(kyc.status)
                            const Icon = meta.icon
                            return (
                                <span className={`settings-kyc-badge ${meta.cls}`}>
                                    <Icon /> {meta.label}
                                </span>
                            )
                        })()}
                    </div>
                    <p className='settings-hero-email'>{user?.email}</p>
                    <div className='settings-hero-stats'>
                        <div>
                            <span className='settings-hero-stat-label'>Member since</span>
                            <span className='settings-hero-stat-value'>{user ? formatMemberSince(user.created_at) : '—'}</span>
                        </div>
                        <div className='settings-hero-stat-divider' />
                        <div>
                            <span className='settings-hero-stat-label'>Credit score</span>
                            <span className='settings-hero-stat-value'>
                                {scoreEligible && creditScore ? <>{creditScore.score}<i>/{CREDIT_SCORE_MAX}</i></> : '—'}
                            </span>
                        </div>
                        <div className='settings-hero-stat-divider' />
                        <div>
                            <span className='settings-hero-stat-label'>User ID</span>
                            <span className='settings-hero-stat-value'>{user ? shortUserId(user.id) : '—'}</span>
                        </div>
                    </div>
                </div>

                {(() => {
                    const current = ACCENT_OPTIONS.find(opt => opt.key === accent) ?? ACCENT_OPTIONS[0]
                    const other = ACCENT_OPTIONS.find(opt => opt.key !== accent) ?? ACCENT_OPTIONS[1]
                    return (
                        <button
                            type='button'
                            className={`settings-hero-accent-toggle is-${current.key}`}
                            role='switch'
                            aria-checked={current.key === 'green'}
                            aria-label={`Workspace accent: ${current.label}. Switch to ${other.label}.`}
                            onClick={() => handleSetAccent(other.key)}
                        >
                            {ACCENT_OPTIONS.map(opt => (
                                <span key={opt.key} className={`settings-hero-accent-label is-${opt.key === ACCENT_OPTIONS[0].key ? 'left' : 'right'}`}>
                                    {opt.label}
                                </span>
                            ))}
                            <span className='settings-hero-accent-thumb'>
                                <current.icon aria-hidden='true' />
                            </span>
                        </button>
                    )
                })()}
            </section>

            {/*
                Four cards, placed by CSS grid-area (not DOM order alone) so
                Wallets — which only renders once scoreEligible — never
                leaves a hole: `.has-wallets` swaps in a layout with a
                "wallets" cell, its absence falls back to Credit spanning
                the full left column. `align-items: stretch` (the grid
                default) makes each row's shorter card match its partner's
                height — Wallets/Identity in row 1, Credit/Account in row 2.
            */}
            <div className={`settings-grid${scoreEligible ? ' has-wallets' : ''}`}>
                {scoreEligible && (
                    <Suspense fallback={
                        <section className='settings-card settings-card-wallets'>
                            <div className='settings-card-head'><ShieldCheck /><h2>Wallets</h2></div>
                            <p className='settings-muted'>Loading wallets…</p>
                        </section>
                    }>
                        <WalletsCard />
                    </Suspense>
                )}

                <section className='settings-card settings-card-identity'>
                    <div className='settings-card-head'><ShieldCheck /><h2>Identity verification</h2></div>
                    {kycLoading ? (
                        <p className='settings-muted'>Loading verification status…</p>
                    ) : kycError || !kyc ? (
                        <p className='settings-muted'>Couldn’t load your verification status. Please try again later.</p>
                    ) : kyc.status === 'none' ? (
                        <div className='settings-kyc-empty'>
                            <p className='settings-muted'>You haven’t submitted identity verification yet.</p>
                            <button type='button' className='settings-btn-primary' onClick={() => navigate('/verification')}>
                                Verify your identity
                            </button>
                        </div>
                    ) : (
                        <>
                            <p className='settings-muted'>
                                {kyc.status === 'verifying' && 'A reviewer is verifying your ID against your selfie. This usually takes 24–48 hours — no action needed from you.'}
                                {kyc.status === 'approved' && 'Your identity has been confirmed. You have full access to lending and borrowing features.'}
                                {kyc.status === 'rejected' && 'We couldn’t verify your submission. Please review the reason below and resubmit.'}
                            </p>
                            <ol className='settings-timeline'>
                                {verifySteps(kyc).map((step, i, arr) => (
                                    <li key={step.title} className={`is-${step.state}`}>
                                        <span className='settings-timeline-dot'>
                                            {step.state === 'done' && <Check aria-hidden='true' />}
                                            {step.state === 'current' && <Clock aria-hidden='true' />}
                                            {step.state === 'rejected' && <X aria-hidden='true' />}
                                        </span>
                                        {i < arr.length - 1 && <span className='settings-timeline-line' />}
                                        <div className='settings-timeline-body'>
                                            <p className='settings-timeline-title'>{step.title}</p>
                                            {step.time && <p className='settings-timeline-time'>{step.time}</p>}
                                        </div>
                                    </li>
                                ))}
                            </ol>
                            {kyc.status === 'rejected' && (
                                <>
                                    {kyc.rejection_reason && <p className='settings-kyc-reason'>Reason: {kyc.rejection_reason}</p>}
                                    <button type='button' className='settings-btn-primary settings-btn-full' onClick={() => navigate('/verification')}>
                                        Resubmit verification
                                    </button>
                                </>
                            )}
                        </>
                    )}
                </section>

                <section className='settings-card settings-card-credit'>
                    <div className='settings-card-head'><Gauge /><h2>Credit score</h2></div>
                    {!scoreEligible ? (
                        <p className='settings-muted'>Your credit score becomes available once your identity is verified.</p>
                    ) : creditLoading ? (
                        <p className='settings-muted'>Loading credit score…</p>
                    ) : creditError || !creditScore ? (
                        <p className='settings-muted'>Couldn’t load your credit score. Please try again later.</p>
                    ) : (
                        <div className='settings-score'>
                            <div className='settings-score-arc'>
                                <svg viewBox='0 0 220 124' width='100%'>
                                    <path d='M14,114 A96,96 0 0 1 206,114' fill='none' stroke='rgba(255,255,255,.08)' strokeWidth='15' strokeLinecap='round' />
                                    <path
                                        d='M14,114 A96,96 0 0 1 206,114'
                                        fill='none'
                                        stroke='var(--auth-primary)'
                                        strokeWidth='15'
                                        strokeLinecap='round'
                                        pathLength={100}
                                        strokeDasharray={`${(creditScore.score / CREDIT_SCORE_MAX) * 100} 100`}
                                    />
                                </svg>
                                <div className='settings-score-arc-value'>
                                    {creditScore.score}<span>/{CREDIT_SCORE_MAX}</span>
                                    <p>{creditTier(creditScore.score).toUpperCase()}</p>
                                </div>
                            </div>
                            <div className='settings-score-axis'>
                                <span>0</span><span>Fair</span><span>Strong</span><span>{CREDIT_SCORE_MAX}</span>
                            </div>
                            <p className='settings-muted'>Every account starts at 50. This will move as your borrowing and repayment history grows.</p>
                        </div>
                    )}
                </section>

                <section className='settings-card settings-card-account'>
                    <div className='settings-card-head'><LogOut /><h2>Account</h2></div>
                    <p className='settings-muted'>Sign out of PrimeLendRow on this device.</p>
                    <button type='button' className='settings-signout' onClick={() => setConfirming(true)}>
                        <LogOut /> Sign out
                    </button>
                </section>
            </div>

            {confirming && (
                <div className='logout-confirm-overlay'>
                    <div className='sidebar-logout-confirm' role='dialog' aria-label='Confirm log out'>
                        <p>Sign out of PrimeLendRow?</p>
                        <div className='sidebar-logout-confirm-actions'>
                            <button type='button' className='sidebar-logout-cancel' onClick={() => setConfirming(false)}>
                                Cancel
                            </button>
                            <button type='button' className='sidebar-logout-confirm-btn' onClick={performLogout}>
                                Sign out
                            </button>
                        </div>
                    </div>
                </div>
            )}

            {loggingOut && (
                <div className='logout-overlay' role='status' aria-live='polite'>
                    <div className='logout-overlay-mark'><img src={LOGO} alt='' /></div>
                    <div className='logout-spinner' aria-hidden='true' />
                    <p>Signing you out…</p>
                </div>
            )}
        </main>
    )
}

export default Settings
