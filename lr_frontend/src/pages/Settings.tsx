import { useCallback, useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { User, ShieldCheck, Palette, LogOut, Bubbles, Leaf, BadgeCheck, Hourglass, CircleAlert, Gauge } from 'lucide-react'

import { useSession } from '../providers/useSession'
import { useToast } from '../providers/useToast'
import { useAccent } from '../providers/AccentProvider'
import { useCreditScore, CREDIT_SCORE_MAX } from '../functions/useCreditScore'

const API = import.meta.env.VITE_API_URL ?? ''
const LOGO = '/pictures/lr.png'
/** Keeps the sign-out overlay on screen at least this long, so a fast response doesn't just flash. */
const LOGOUT_MIN_DISPLAY_MS = 550

type KycStatusValue = 'none' | 'pending' | 'approved' | 'rejected'
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
const ROLE_LABEL: Record<string, string> = { Admin: 'Admin', User: 'Verified', Pending: 'Pending' }

const KYC_LABEL: Record<KycStatusValue, string> = {
    none: 'Not submitted',
    pending: 'Under review',
    approved: 'Approved',
    rejected: 'Rejected',
}

// created_at/reviewed_at are unix seconds (Utc::now().timestamp() server-side)
const formatDate = (secs: number | null) =>
    secs == null ? '' : new Date(secs * 1000).toLocaleString(undefined, {
        year: 'numeric', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit',
    })

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

    const roleLabel = user ? (ROLE_LABEL[user.role] ?? user.role) : ''

    return (
        <main className='settings-page'>
            <header className='settings-head'>
                <h1>Settings</h1>
                <p>Manage your profile, verification, and appearance.</p>
            </header>

            <section className='settings-card'>
                <div className='settings-card-head'><User /><h2>Profile</h2></div>
                <div className='settings-profile'>
                    <div className='settings-avatar'>{initialsOf(user?.username ?? '')}</div>
                    <div className='settings-profile-id'>
                        <p className='settings-profile-name'>{user?.username}</p>
                        <p className='settings-profile-email'>{user?.email}</p>
                    </div>
                    <span className={`settings-role-chip is-${(user?.role ?? '').toLowerCase()}`}>{roleLabel}</span>
                </div>
                <dl className='settings-fields'>
                    <div><dt>Username</dt><dd>{user?.username || '—'}</dd></div>
                    <div><dt>Email</dt><dd>{user?.email || '—'}</dd></div>
                    <div><dt>Account role</dt><dd>{roleLabel || '—'}</dd></div>
                </dl>
            </section>

            <section className='settings-card'>
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
                        <div className='settings-kyc-status'>
                            <span className={`settings-kyc-badge is-${kyc.status}`}>
                                {kyc.status === 'approved' && <BadgeCheck />}
                                {kyc.status === 'pending' && <Hourglass />}
                                {kyc.status === 'rejected' && <CircleAlert />}
                                {KYC_LABEL[kyc.status]}
                            </span>
                        </div>
                        <dl className='settings-fields'>
                            {kyc.submitted_at != null && <div><dt>Submitted</dt><dd>{formatDate(kyc.submitted_at)}</dd></div>}
                            {kyc.reviewed_at != null && <div><dt>Reviewed</dt><dd>{formatDate(kyc.reviewed_at)}</dd></div>}
                        </dl>
                        {kyc.status === 'rejected' && (
                            <>
                                {kyc.rejection_reason && <p className='settings-kyc-reason'>Reason: {kyc.rejection_reason}</p>}
                                <button type='button' className='settings-btn-primary' onClick={() => navigate('/verification')}>
                                    Resubmit verification
                                </button>
                            </>
                        )}
                    </>
                )}
            </section>

            <section className='settings-card'>
                <div className='settings-card-head'><Gauge /><h2>Credit score</h2></div>
                {creditLoading ? (
                    <p className='settings-muted'>Loading credit score…</p>
                ) : creditError || !creditScore ? (
                    <p className='settings-muted'>Couldn’t load your credit score. Please try again later.</p>
                ) : (
                    <div className='settings-score'>
                        <div className='settings-score-value'>
                            {creditScore.score}<span>/{CREDIT_SCORE_MAX}</span>
                        </div>
                        <div className='settings-score-track'>
                            <div
                                className='settings-score-fill'
                                style={{ width: `${(creditScore.score / CREDIT_SCORE_MAX) * 100}%` }}
                            />
                        </div>
                        <p className='settings-muted'>Every account starts at 50. This will move as your borrowing and repayment history grows.</p>
                    </div>
                )}
            </section>

            <section className='settings-card'>
                <div className='settings-card-head'><Palette /><h2>Appearance</h2></div>
                <p className='settings-muted'>Accent color</p>
                <div className='settings-accent'>
                    <button
                        type='button'
                        className={`settings-accent-option${accent === 'blue' ? ' is-active' : ''}`}
                        aria-pressed={accent === 'blue'}
                        onClick={() => setAccent('blue')}
                    >
                        <Bubbles /> Ocean
                    </button>
                    <button
                        type='button'
                        className={`settings-accent-option${accent === 'green' ? ' is-active' : ''}`}
                        aria-pressed={accent === 'green'}
                        onClick={() => setAccent('green')}
                    >
                        <Leaf /> Forest
                    </button>
                </div>
            </section>

            <section className='settings-card'>
                <div className='settings-card-head'><LogOut /><h2>Account</h2></div>
                <p className='settings-muted'>Sign out of PrimeLendRow on this device.</p>
                <button type='button' className='settings-signout' onClick={() => setConfirming(true)}>
                    <LogOut /> Sign out
                </button>
            </section>

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
