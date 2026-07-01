import { useEffect, useMemo, useState } from 'react'
import type { ReactNode } from 'react'
import { useToast } from './useToast'
import { SessionContext, type SessionPayload } from './useSession'

interface SessionProviderProps {
    children: ReactNode
}

export const SessionProvider = ({ children }: SessionProviderProps) => {
    const [loading, setLoading] = useState(true)
    const [user, setUser] = useState<SessionPayload | null>(null)
    const [userDetails, setUserDetails] = useState<unknown>(null)
    const [csrfToken, setCsrfToken] = useState<string | null>(null)

    const toast = useToast()

    useEffect(() => {
        let aborted = false

        const ENGINE = import.meta.env.VITE_API_URL

        const run = async () => {
            setLoading(true)
            try {
                const res = await fetch(`${ENGINE}/auth/session`, {
                    method: 'GET',
                    credentials: 'include',
                    headers: { 'Content-Type': 'application/json' }
                })

                if (!res.ok) {
                    if (!aborted) {
                        setUser(null)
                        setCsrfToken(null)
                    }
                    return
                }

                const text = await res.text()
                const nextUser = text ? JSON.parse(text) as SessionPayload : null

                if (!nextUser?.username || !nextUser?.email || !nextUser?.role) {
                    if (!aborted) {
                        setUser(null)
                        setCsrfToken(null)
                    }
                    return
                }

                if (!aborted) {
                    setUser(nextUser)
                    setCsrfToken(res.headers.get('x-csrf-token'))
                }
            } catch (err) {
                toast.error(err instanceof Error ? err.message : 'Unable to refresh session')
                if (!aborted) {
                    setUser(null)
                    setCsrfToken(null)
                }
            } finally {
                if (!aborted) setLoading(false)
            }
        }

        run()

        const onRefresh = () => { if (!aborted) run() }
        window.addEventListener('session:refresh', onRefresh)

        return () => {
            aborted = true
            window.removeEventListener('session:refresh', onRefresh)
        }
    }, [toast])

    const value = useMemo(() => ({
        loading,
        user,
        userDetails,
        csrfToken,
        setUser,
        setUserDetails,
        setCsrfToken,
    }), [loading, user, userDetails, csrfToken])

    return (
        <SessionContext.Provider value={value}>
            {children}
        </SessionContext.Provider>
    )
}
