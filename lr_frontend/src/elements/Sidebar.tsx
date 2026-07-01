import { useState } from 'react'
import { BetweenHorizontalStart, BetweenHorizontalEnd, LayoutGrid, Landmark, ClipboardList, CreditCard, Settings, LogOut, type LucideIcon } from 'lucide-react'

import { useSession } from '../providers/useSession'
import { useToast } from '../providers/useToast'

const LOGO = '/pictures/lr.png'

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
    const toast = useToast()

    const logout = async () => {
        try {
            const ENGINE = import.meta.env.VITE_API_URL ?? ''
            const res = await fetch(`${ENGINE}/auth/logout`, {
                method: 'POST',
                credentials: 'include',
                headers: csrfToken ? { 'x-csrf-token': csrfToken } : undefined,
            })
            if (!res.ok) throw new Error((await res.text()) || 'Unable to log out')
            setUser(null)
        } catch (err) {
            toast.error(err instanceof Error ? err.message : 'Unable to log out')
        }
    }

    const verifyAccount = () => {
        toast.info('Account verification isn\'t available yet — check back soon')
    }

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

                    return (
                    <button
                        key={item.key}
                        type='button'
                        title={item.label}
                        className={`sidebar-item${active === item.key ? ' is-active' : ''}`}
                        onClick={() => setActive(item.key)}
                    >
                        <Icon size={20} />
                        {!collapsed && <span>{item.label}</span>}
                    </button>
                    )
                })}
            </nav>

            {!collapsed && user?.role === 'Pending' && (
                <div className='sidebar-status'>
                    <div className='sidebar-status-head'>
                        <i className='bx bxs-hourglass' />
                        <span>Current Account Status</span>
                    </div>
                    <p className='sidebar-status-value'>Pending</p>
                    <p className='sidebar-status-desc'>Update information to access latest and advanced features</p>
                    <button type='button' className='sidebar-status-btn' onClick={verifyAccount}>
                        <i className='bx bx-check-shield' />
                        Verify Account
                    </button>
                </div>
            )}

            <div className='sidebar-user'>
                <div className='sidebar-avatar'>{initialsOf(user?.username ?? '')}</div>
                {!collapsed && (
                    <>
                        <div className='sidebar-user-info'>
                            <p className='sidebar-user-name'>{user?.username}</p>
                            <p className='sidebar-user-role'>{user?.role}</p>
                        </div>
                        <button type='button' className='sidebar-logout' aria-label='Log out' onClick={logout}>
                            <LogOut />
                        </button>
                    </>
                )}
            </div>
        </aside>
    )
}

export default Sidebar
