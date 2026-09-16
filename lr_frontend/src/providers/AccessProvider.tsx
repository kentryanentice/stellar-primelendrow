import type { ReactNode } from 'react'
import { Navigate, useLocation } from 'react-router-dom'
import { useSession } from './useSession'

interface Props {
    children: ReactNode
}

const PUBLIC_ROUTES = ['/', '/auth']
/** Open to everyone, signed in or not, with no redirect either way. */
const OPEN_ROUTES = ['/records']
const ADMIN_ROUTES = ['/dashboard', '/admin', '/settings', '/lending', '/borrow', '/pay']
const USER_ROUTES = ['/dashboard', '/settings', '/verification', '/lending', '/borrow', '/pay']

function AccessProvider({ children }: Props) {
    const { loading, user } = useSession()
    const location = useLocation()
    const { pathname } = location

    // Before the session check: these pages don't depend on who is looking,
    // so they needn't wait for the session to load either.
    if (OPEN_ROUTES.some(r => pathname === r || pathname.startsWith(`${r}/`))) return <>{children}</>

    if (loading && !user) return <div className='loader' />

    if (!user) {
        if (PUBLIC_ROUTES.includes(pathname)) return <>{children}</>
        return <Navigate to='/auth' replace state={{ from: location }} />
    }

    if (user.role === 'Admin') {
        if (ADMIN_ROUTES.some(r => pathname.startsWith(r))) return <>{children}</>
        return <Navigate to='/dashboard' replace />
    }

    if (user.role === 'User' || user.role === 'Pending' || user.role === 'Verifying') {
        if (USER_ROUTES.some(r => pathname.startsWith(r))) return <>{children}</>
        return <Navigate to='/dashboard' replace />
    }

    return <Navigate to='/' replace />
}

export default AccessProvider
