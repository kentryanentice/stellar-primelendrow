import { useCallback, useEffect, useState } from 'react'
import { useSession } from '../../providers/useSession'
import { useToast } from '../../providers/useToast'
import { connectFreighter, signChallenge } from './wallet'

const API = import.meta.env.VITE_API_URL ?? ''

export type Wallet = {
    id: string
    address: string
    label: string | null
    /** "kyc_verified" | "user_added" */
    source: string
    /** "active" | "disconnected" */
    status: string
    connected_at: number
    disconnected_at: number | null
}

type ChallengeResponse = { nonce: string; message: string; expires_at: number }

/**
 * Drives the "Wallets" settings card: loads the caller's wallet list, and
 * exposes connect (challenge -> sign -> connect) and disconnect. Naming is
 * `use`-prefixed per this repo's React Compiler requirement — a plain
 * function here would have its internal state silently frozen.
 */
export default function useWallets() {
    const { csrfToken } = useSession()
    const toast = useToast()

    const [wallets, setWallets] = useState<Wallet[]>([])
    const [loading, setLoading] = useState(true)
    const [error, setError] = useState(false)
    const [connecting, setConnecting] = useState(false)
    const [disconnectingId, setDisconnectingId] = useState<string | null>(null)

    const authHeaders = useCallback((): HeadersInit => ({
        'Content-Type': 'application/json',
        ...(csrfToken ? { 'x-csrf-token': csrfToken } : {}),
    }), [csrfToken])

    const refresh = useCallback(async () => {
        setLoading(true)
        setError(false)
        try {
            const res = await fetch(`${API}/wallets`, { credentials: 'include' })
            if (!res.ok) throw new Error()
            const data = await res.json() as { wallets: Wallet[] }
            setWallets(data.wallets)
        } catch {
            setError(true)
        } finally {
            setLoading(false)
        }
    }, [])

    useEffect(() => {
        void refresh()
    }, [refresh])

    // connect -> challenge -> sign -> connect: the backend never trusts an
    // address unless the wallet just proved it holds the private key behind
    // it (SEP-0053 signature over a one-time server-issued nonce).
    const connectNewWallet = useCallback(async (label?: string) => {
        setConnecting(true)
        try {
            const connectResult = await connectFreighter()
            if ('error' in connectResult) {
                toast.error(connectResult.error)
                return
            }
            const { address } = connectResult

            const challengeRes = await fetch(`${API}/wallets/challenge`, {
                method: 'POST',
                credentials: 'include',
                headers: authHeaders(),
            })
            if (!challengeRes.ok) throw new Error(await challengeRes.text() || 'Unable to start wallet verification')
            const { nonce, message } = await challengeRes.json() as ChallengeResponse

            const signResult = await signChallenge(message, address)
            if ('error' in signResult) {
                toast.error(signResult.error)
                return
            }

            const connectRes = await fetch(`${API}/wallets/connect`, {
                method: 'POST',
                credentials: 'include',
                headers: authHeaders(),
                body: JSON.stringify({ nonce, address, signature: signResult.signature, label: label?.trim() || undefined }),
            })
            if (!connectRes.ok) throw new Error(await connectRes.text() || 'Unable to connect wallet')

            toast.success('Wallet connected')
            await refresh()
        } catch (err) {
            toast.error(err instanceof Error ? err.message : 'Unable to connect wallet')
        } finally {
            setConnecting(false)
        }
    }, [authHeaders, refresh, toast])

    const disconnectWallet = useCallback(async (walletId: string) => {
        setDisconnectingId(walletId)
        try {
            const res = await fetch(`${API}/wallets/disconnect`, {
                method: 'POST',
                credentials: 'include',
                headers: authHeaders(),
                body: JSON.stringify({ wallet_id: walletId }),
            })
            if (!res.ok) throw new Error(await res.text() || 'Unable to disconnect wallet')
            toast.success('Wallet disconnected')
            await refresh()
        } catch (err) {
            toast.error(err instanceof Error ? err.message : 'Unable to disconnect wallet')
        } finally {
            setDisconnectingId(null)
        }
    }, [authHeaders, refresh, toast])

    return { wallets, loading, error, connecting, disconnectingId, connectNewWallet, disconnectWallet }
}
