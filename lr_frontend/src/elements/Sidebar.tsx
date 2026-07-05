import { useCallback, useEffect, useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { BetweenHorizontalStart, BetweenHorizontalEnd, LayoutGrid, Landmark, ClipboardList, CreditCard, Settings, LogOut, Hourglass, Lock, type LucideIcon } from 'lucide-react'

import { useSession } from '../providers/useSession'
import { useToast } from '../providers/useToast'

const LOGO = '/pictures/lr.png'
/** Keeps the sign-out overlay on screen at least this long, so a fast response doesn't just flash. */
const LOGOUT_MIN_DISPLAY_MS = 550
/** How long the pending-status card glows after a locked nav item is clicked. */
const STATUS_PULSE_MS = 900

type NavKey = 'dashboard' | 'lend' | 'borrow' | 'pay' | 'settings'

type NavItem = {
  key: NavKey
  label: string
  icon: LucideIcon
}

const NAV_ITEMS: NavItem[] = [
  { key: 'dashboard', label: 'Dashboard', icon: LayoutGrid },
  { key: 'lend', label: 'Lend', icon: Landmark },
  { key: 'borrow', label: 'Borrow', icon: ClipboardList },
  { key: 'pay', label: 'Pay', icon: CreditCard },
  { key: 'settings', label: 'Settings', icon: Settings },
]
/** Every nav item locks for a Pending account except Settings. */
const UNLOCKED_WHEN_PENDING: NavKey[] = ['settings']

const initialsOf = (name: string) => {
    const parts = name.trim().split(/\s+/).filter(Boolean)
    if (!parts.length) return '?'
    const first = parts[0][0]
    const last = parts.length > 1 ? parts[parts.length - 1][0] : parts[0][1]
    return `${first}${last ?? ''}`.toUpperCase()
}

interface SidebarProps {
    collapsed: boolean
    onToggleCollapsed: () => void
}

function Sidebar({ collapsed, onToggleCollapsed }: SidebarProps) {
    const { user, setUser, csrfToken } = useSession()
    const [active, setActive] = useState<NavKey>('dashboard')
    const [statusPulsing, setStatusPulsing] = useState(false)
    const [confirmingLogout, setConfirmingLogout] = useState(false)
    const [loggingOut, setLoggingOut] = useState(false)
    const toast = useToast()
    const navigate = useNavigate()

    const pulseTimer = useRef<number | undefined>(undefined)
    const userRowRef = useRef<HTMLDivElement>(null)

    const isPending = user?.role === 'Pending'
    // the confirm dialog and the signing-out overlay already visually block
    // the rest of the sidebar (full-screen, higher z-index) — this closes the
    // remaining gap where an already-focused nav button could still be
    // activated from the keyboard underneath them
    const disableOthers = confirmingLogout || loggingOut

    useEffect(() => () => {
        if (pulseTimer.current) window.clearTimeout(pulseTimer.current)
    }, [])

    // lock the page behind the logout confirm modal so it can't be scrolled or clicked
    useEffect(() => {
        if (!confirmingLogout) return
        const prevOverflow = document.body.style.overflow
        document.body.style.overflow = 'hidden'
        return () => { document.body.style.overflow = prevOverflow }
    }, [confirmingLogout])

    const verifyAccount = useCallback(() => {
        navigate('/verification')
    }, [navigate])

    // clicking a locked item can't navigate anywhere useful yet, so it nudges
    // the user toward the fix instead: a toast, and — expanding the sidebar
    // first if needed — a glow pulse on the pending-status card and its
    // existing "Verify Account" button
    const promptLocked = useCallback((label: string) => {
        toast.info(`Verify your identity to unlock ${label}`)
        if (collapsed) onToggleCollapsed()
        if (pulseTimer.current) window.clearTimeout(pulseTimer.current)
        setStatusPulsing(false)
        window.requestAnimationFrame(() => {
            setStatusPulsing(true)
            pulseTimer.current = window.setTimeout(() => setStatusPulsing(false), STATUS_PULSE_MS)
        })
    }, [collapsed, onToggleCollapsed, toast])

    const performLogout = useCallback(async () => {
        setConfirmingLogout(false)
        setLoggingOut(true)
        try {
            const ENGINE = import.meta.env.VITE_API_URL ?? ''
            const minDisplay = new Promise(resolve => window.setTimeout(resolve, LOGOUT_MIN_DISPLAY_MS))
            const [res] = await Promise.all([
                fetch(`${ENGINE}/auth/logout`, {
                    method: 'POST',
                    credentials: 'include',
                    headers: csrfToken ? { 'x-csrf-token': csrfToken } : undefined,
                }),
                minDisplay,
            ])
            if (!res.ok) throw new Error((await res.text()) || 'Unable to log out')
            // AccessProvider redirects to /auth as soon as user is cleared — this
            // component unmounts right after, taking the overlay with it
            setUser(null)
        } catch (err) {
            setLoggingOut(false)
            toast.error(err instanceof Error ? err.message : 'Unable to log out')
        }
    }, [csrfToken, setUser, toast])

    return (
        <aside className={`sidebar${collapsed ? ' is-collapsed' : ''}`}>
            <button type='button' className='sidebar-toggle' aria-label='Toggle sidebar' onClick={onToggleCollapsed} disabled={disableOthers}>
                {collapsed ? <BetweenHorizontalStart /> : <BetweenHorizontalEnd /> }
            </button>

            <div className='sidebar-brand'>
                <img src={LOGO} alt='' />
                {!collapsed && <span>Prime<b>LendRow</b></span>}
            </div>

            <nav className='sidebar-nav'>
                {NAV_ITEMS.map(item => {
                    const Icon = item.icon
                    const locked = isPending && !UNLOCKED_WHEN_PENDING.includes(item.key)

                    return (
                    <button
                        key={item.key}
                        type='button'
                        title={locked ? `${item.label} — verify your identity to unlock` : item.label}
                        aria-disabled={locked}
                        disabled={disableOthers}
                        className={`sidebar-item${active === item.key ? ' is-active' : ''}${locked ? ' is-locked' : ''}`}
                        onClick={() => (locked ? promptLocked(item.label) : setActive(item.key))}
                    >
                        <span className='sidebar-item-icon'>
                            <Icon size={20} />
                            {locked && <Lock size={11} className='sidebar-item-lock' aria-hidden='true' />}
                        </span>
                        {!collapsed && <span>{item.label}</span>}
                    </button>
                    )
                })}
            </nav>

            {!collapsed && isPending && (
                <div className={`sidebar-status${statusPulsing ? ' is-pulsing' : ''}`}>
                    <div className='sidebar-status-head'>
                        <Hourglass />
                        <span>Current Account Status</span>
                    </div>
                    <p className='sidebar-status-value'>Pending</p>
                    <p className='sidebar-status-desc'>Complete identity verification to access latest and advanced features</p>
                    <button type='button' className='sidebar-status-btn' onClick={verifyAccount} disabled={disableOthers}>
                        Verify Account
                    </button>
                </div>
            )}

            <div className='sidebar-user' ref={userRowRef}>
                <div className='sidebar-avatar'>{initialsOf(user?.username ?? '')}</div>
                {!collapsed && (
                    <>
                        <div className='sidebar-user-info'>
                            <p className='sidebar-user-name'>{user?.username}</p>
                            <p className='sidebar-user-role'>{user?.role}</p>
                        </div>
                        <button
                            type='button'
                            className='sidebar-logout'
                            aria-label='Log out'
                            aria-expanded={confirmingLogout}
                            disabled={disableOthers}
                            onClick={() => setConfirmingLogout(v => !v)}
                        >
                            <LogOut />
                        </button>
                        {confirmingLogout && (
                            <div className='logout-confirm-overlay'>
                                <div
                                    className='sidebar-logout-confirm'
                                    role='dialog'
                                    aria-label='Confirm log out'
                                >
                                    <p>Sign out of PrimeLendRow?</p>
                                    <div className='sidebar-logout-confirm-actions'>
                                        <button type='button' className='sidebar-logout-cancel' onClick={() => setConfirmingLogout(false)}>
                                            Cancel
                                        </button>
                                        <button type='button' className='sidebar-logout-confirm-btn' onClick={performLogout}>
                                            Sign out
                                        </button>
                                    </div>
                                </div>
                            </div>
                        )}
                    </>
                )}
            </div>

            {loggingOut && (
                <div className='logout-overlay' role='status' aria-live='polite'>
                    <div className='logout-overlay-mark'>
                        <img src={LOGO} alt='' />
                    </div>
                    <div className='logout-spinner' aria-hidden='true' />
                    <p>Signing you out…</p>
                </div>
            )}
        </aside>
    )
}

export default Sidebar
