import { useCallback, useEffect, useState } from 'react'
import { useSession } from '../../providers/useSession'
import { useToast } from '../../providers/useToast'
import type { Invite } from './types'
import { apiFetch } from '../apiFetch'

const API = import.meta.env.VITE_API_URL ?? ''

/** GET /guarantors/invites — reads only; `refresh` and the first-load effect apply it. */
async function fetchInvites(signal?: AbortSignal) {
    const res = await apiFetch(`${API}/guarantors/invites`, { credentials: 'include', signal })
    if (!res.ok) throw new Error()
    const data = await res.json() as { invites: Invite[] }
    return data.invites
}

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
            setInvites(await fetchInvites())
        } catch {
            setError(true)
        } finally {
            setLoading(false)
        }
    }, [])

    // First load. The initial state already says "loading", so state is only
    // written once the response is in — never synchronously in the effect.
    useEffect(() => {
        const controller = new AbortController()
        void (async () => {
            try {
                const invites = await fetchInvites(controller.signal)
                if (!controller.signal.aborted) setInvites(invites)
            } catch {
                if (!controller.signal.aborted) setError(true)
            } finally {
                if (!controller.signal.aborted) setLoading(false)
            }
        })()
        return () => controller.abort()
    }, [])

    const respond = useCallback(async (inviteId: string, accept: boolean) => {
        setRespondingId(inviteId)
        try {
            const res = await apiFetch(`${API}/guarantors/respond`, {
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
