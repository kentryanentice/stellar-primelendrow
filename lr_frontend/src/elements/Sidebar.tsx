import { useCallback, useEffect, useRef, useState } from 'react'
import { useNavigate, useLocation } from 'react-router-dom'
import { BetweenHorizontalStart, BetweenHorizontalEnd, LayoutGrid, Landmark, ClipboardList, CreditCard, Settings, Hourglass, Lock, ShieldCheck, BadgeCheck, Gauge, type LucideIcon } from 'lucide-react'

import { useSession } from '../providers/useSession'
import { useToast } from '../providers/useToast'
import { useCreditScore, CREDIT_SCORE_MAX } from '../functions/useCreditScore'

const LOGO = '/pictures/lr.png'
/** How long the pending-status card glows after a locked nav item is clicked. */
const STATUS_PULSE_MS = 900

type NavKey = 'dashboard' | 'lend' | 'borrow' | 'pay' | 'settings'

type NavItem = {
  key: NavKey
  label: string
  icon: LucideIcon
  /** Items with a route navigate; the rest are placeholders that only highlight. */
  route?: string
}

const NAV_ITEMS: NavItem[] = [
  { key: 'dashboard', label: 'Dashboard', icon: LayoutGrid, route: '/dashboard' },
  { key: 'lend', label: 'Lend', icon: Landmark },
  { key: 'borrow', label: 'Borrow', icon: ClipboardList },
  { key: 'pay', label: 'Pay', icon: CreditCard },
  { key: 'settings', label: 'Settings', icon: Settings, route: '/settings' },
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

const firstNameOf = (name: string) => name.trim().split(/\s+/)[0] ?? ''

/** The status echoed as a themed pip on the avatar (the text line stays too). 'User' is a
 *  KYC-passed member, surfaced as "Verified"; anything unrecognised gets no pip. */
const STATUS_BADGE: Record<string, { label: string; icon: LucideIcon; cls: string }> = {
    Admin: { label: 'Admin', icon: ShieldCheck, cls: 'is-admin' },
    User: { label: 'Verified', icon: BadgeCheck, cls: 'is-verified' },
    Verified: { label: 'Verified', icon: BadgeCheck, cls: 'is-verified' },
    Pending: { label: 'Pending', icon: Hourglass, cls: 'is-pending' },
}

/** Always-valid openers; the time-of-day line is added separately so it can track the clock. */
const GREETINGS = [
  'Welcome back',
  'Hello there',
  'Hi',
  'Good to see you',
  'How’s your day',
  'What’s up',
  'Nice seeing you',
  'Hey there',
  'Welcome',
  'Greetings',
  'Howdy',
  'Hey',
  'Hi again',
  'Welcome again',
  'Glad you’re here',
  'Nice to see you',
  'Happy to see you',
  'Great seeing you',
  'Good day',
  'Hello again',
  'Deym',
  'Ready to begin',
  'Back again',
  'Welcome aboard',
  'Nice to meet',
  'Hope all’s well',
  'How are you',
  'What’s new',
  'Good vibes',
  'Great day',
  'Hey friend',
  'Hi friend',
  'Welcome friend',
  'Happy to help',
  'Ready to go',
  'Let’s begin',
  'Glad you’re back',
  'Good seeing you',
  'Welcome home',
  'Hello friend',
  'Hey again',
  'Nice return',
  'Good return'
]

const timeGreeting = (hour: number) => (hour < 12 ? 'Good morning' : hour < 18 ? 'Good afternoon' : 'Good evening')

/** The clock-aware option is swapped in for the current part of day so "Good morning" never
 *  shows at night; everything else is always in the pool. */
const pickGreeting = () => {
    const pool = [...GREETINGS, timeGreeting(new Date().getHours())]
    return pool[Math.floor(Math.random() * pool.length)]
}

interface SidebarProps {
    collapsed: boolean
    onToggleCollapsed: () => void
}

function Sidebar({ collapsed, onToggleCollapsed }: SidebarProps) {
    const { user } = useSession()
    const { score: creditScore } = useCreditScore()
    // re-picked on every mount/reload so the greeting rotates each visit; stays put
    // across re-renders within a session (pulses, navigation)
    const [greeting] = useState(pickGreeting)
    const [active, setActive] = useState<NavKey>('dashboard')
    const [statusPulsing, setStatusPulsing] = useState(false)
    const toast = useToast()
    const navigate = useNavigate()
    const { pathname } = useLocation()

    const pulseTimer = useRef<number | undefined>(undefined)

    const isPending = user?.role === 'Pending'
    const badge = user ? STATUS_BADGE[user.role] : undefined
    // the routed page drives the highlight; placeholder items (lend/borrow/pay) fall
    // back to the last local selection since they don't navigate anywhere yet
    const activeKey = NAV_ITEMS.find(item => item.route && pathname.startsWith(item.route))?.key ?? active

    useEffect(() => () => {
        if (pulseTimer.current) window.clearTimeout(pulseTimer.current)
    }, [])

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

    return (
        <aside className={`sidebar${collapsed ? ' is-collapsed' : ''}`}>
            <button type='button' className='sidebar-toggle' aria-label='Toggle sidebar' onClick={onToggleCollapsed}>
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
                        className={`sidebar-item${activeKey === item.key ? ' is-active' : ''}${locked ? ' is-locked' : ''}`}
                        onClick={() => {
                            if (locked) return promptLocked(item.label)
                            if (item.route) navigate(item.route)
                            else setActive(item.key)
                        }}
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

            {!collapsed && creditScore && (
                <div className='sidebar-score'>
                    <div className='sidebar-score-head'>
                        <Gauge />
                        <span>Credit score</span>
                        <span className='sidebar-score-value'>{creditScore.score}<i>/{CREDIT_SCORE_MAX}</i></span>
                    </div>
                    <div className='sidebar-score-track'>
                        <div
                            className='sidebar-score-fill'
                            style={{ width: `${(creditScore.score / CREDIT_SCORE_MAX) * 100}%` }}
                        />
                    </div>
                </div>
            )}

            {!collapsed && isPending && (
                <div className={`sidebar-status${statusPulsing ? ' is-pulsing' : ''}`}>
                    <div className='sidebar-status-head'>
                        <Hourglass />
                        <span>Current Account Status</span>
                    </div>
                    <p className='sidebar-status-value'>Pending</p>
                    <p className='sidebar-status-desc'>Complete identity verification to access latest and advanced features</p>
                    <button type='button' className='sidebar-status-btn' onClick={verifyAccount}>
                        Verify Account
                    </button>
                </div>
            )}

            <div className='sidebar-user'>
                <div className='sidebar-avatar'>
                    {initialsOf(user?.username ?? '')}
                    {badge && (
                        <span className={`sidebar-avatar-badge ${badge.cls}`} title={badge.label} aria-label={badge.label}>
                            <badge.icon aria-hidden='true' />
                        </span>
                    )}
                </div>
                {!collapsed && (
                    <div className='sidebar-user-info'>
                        {(() => {
                            const first = firstNameOf(user?.username ?? '')
                            const line = first ? `${greeting}, ${first}!` : `${greeting}!`
                            return <p className='sidebar-user-name' title={line}>{line}</p>
                        })()}
                        <p className='sidebar-user-role'>{user?.role}</p>
                    </div>
                )}
            </div>
        </aside>
    )
}

export default Sidebar
