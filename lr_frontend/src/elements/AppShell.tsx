import { useState } from 'react'
import type { ReactNode } from 'react'
import Sidebar from './Sidebar'

interface AppShellProps {
    children: ReactNode
}

// matches core/_breakpoints.scss $tablets — everything else about the
// responsive rail/hidden behavior below that width lives in _sidebar.scss
const TABLET_QUERY = '(max-width: 768px)'

/** Sidebar + collapse state shared by every page behind the sidebar (dashboard, verification, …). */
function AppShell({ children }: AppShellProps) {
    const [collapsed, setCollapsed] = useState(() =>
        typeof window !== 'undefined' && window.matchMedia(TABLET_QUERY).matches
    )

    return (
        <div className='app-shell'>
            <Sidebar collapsed={collapsed} onToggleCollapsed={() => setCollapsed(v => !v)} />
            <div className={`app-shell-main${collapsed ? ' is-collapsed' : ''}`}>
                {children}
            </div>
        </div>
    )
}

export default AppShell
