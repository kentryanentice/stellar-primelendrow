import { useCallback, useEffect, useState } from 'react'
import { useSession } from '../../providers/useSession'
import { useToast } from '../../providers/useToast'
import type { Invite } from './types'

const API = import.meta.env.VITE_API_URL ?? ''

/**
 * Invitations to guarantee someone else's loan. Accepting freezes the pledge
 * out of the caller's withdrawable deposits server-side — the consent copy in
 * the card states exactly that before the button is pressed (D1).
 * `use`-prefixed per this repo's React Compiler requirement.
 */
export default function useGuarantorInvites(onChanged: () => void) {
    const { csrfToken } = useSession()
    const toast = useToast()

    const [invites, setInvites] = useState<Invite[]>([])
    const [loading, setLoading] = useState(true)
    const [error, setError] = useState(false)
    const [respondingId, setRespondingId] = useState<string | null>(null)

    const refresh = useCallback(async () => {
        setError(false)
        try {
            const res = await fetch(`${API}/guarantors/invites`, { credentials: 'include' })
            if (!res.ok) throw new Error()
            const data = await res.json() as { invites: Invite[] }
            setInvites(data.invites)
        } catch {
            setError(true)
        } finally {
            setLoading(false)
        }
    }, [])

    useEffect(() => {
        void refresh()
    }, [refresh])

    const respond = useCallback(async (inviteId: string, accept: boolean) => {
        setRespondingId(inviteId)
        try {
            const res = await fetch(`${API}/guarantors/respond`, {
                method: 'POST',
                credentials: 'include',
                headers: {
                    'Content-Type': 'application/json',
                    ...(csrfToken ? { 'x-csrf-token': csrfToken } : {}),
                },
                body: JSON.stringify({ invite_id: inviteId, accept }),
            })
            if (!res.ok) throw new Error(await res.text() || 'Unable to respond to the invitation')
            const data = await res.json() as { message: string }
            toast.success(data.message)
            await refresh()
            onChanged()
        } catch (err) {
            toast.error(err instanceof Error ? err.message : 'Unable to respond to the invitation')
        } finally {
            setRespondingId(null)
        }
    }, [csrfToken, onChanged, refresh, toast])

    return { invites, loading, error, refresh, respond, respondingId }
}
