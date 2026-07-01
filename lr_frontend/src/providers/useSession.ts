import { createContext, useContext } from 'react'
import type { Dispatch, SetStateAction } from 'react'

export interface SessionPayload {
    id: string
    email: string
    username: string
    role: 'Admin' | 'User' | 'Pending' | string
    expires_at: number
}

export interface SessionContextValue {
    loading: boolean
    user: SessionPayload | null
    userDetails: unknown
    csrfToken: string | null
    setUser: Dispatch<SetStateAction<SessionPayload | null>>
    setUserDetails: Dispatch<SetStateAction<unknown>>
    setCsrfToken: Dispatch<SetStateAction<string | null>>
}

export const SessionContext = createContext<SessionContextValue | undefined>(undefined)

export const useSession = (): SessionContextValue => {
    const ctx = useContext(SessionContext)
    if (!ctx) throw new Error('useSession must be used within SessionProvider')
    return ctx
}
