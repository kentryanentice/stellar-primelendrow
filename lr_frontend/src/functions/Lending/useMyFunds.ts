import { useCallback, useState } from 'react'
import { useSession } from '../../providers/useSession'
import { useToast } from '../../providers/useToast'

const API = import.meta.env.VITE_API_URL ?? ''

/**
 * Deposit + withdraw against the pool. The deposit path receives a PayPal
 * order id from the Buttons flow and hands it to the engine, which captures
 * server-side and credits whatever PayPal actually confirms — the amount on
 * screen is never what gets credited, the capture is. `use`-prefixed per
 * this repo's React Compiler requirement.
 */
export default function useMyFunds(onChanged: () => void) {
    const { csrfToken } = useSession()
    const toast = useToast()

    const [confirming, setConfirming] = useState(false)
    const [withdrawing, setWithdrawing] = useState(false)

    const authHeaders = useCallback((): HeadersInit => ({
        'Content-Type': 'application/json',
        ...(csrfToken ? { 'x-csrf-token': csrfToken } : {}),
    }), [csrfToken])

    /** Called from PayPal's onApprove with the approved order id. */
    const confirmDeposit = useCallback(async (orderId: string) => {
        setConfirming(true)
        try {
            const res = await fetch(`${API}/pool/deposit`, {
                method: 'POST',
                credentials: 'include',
                headers: authHeaders(),
                body: JSON.stringify({ order_id: orderId }),
            })
            if (!res.ok) throw new Error(await res.text() || 'Unable to confirm your deposit')
            const data = await res.json() as { message: string }
            toast.success(data.message)
            onChanged()
        } catch (err) {
            toast.error(err instanceof Error ? err.message : 'Unable to confirm your deposit')
        } finally {
            setConfirming(false)
        }
    }, [authHeaders, onChanged, toast])

    const withdraw = useCallback(async (centavos: number) => {
        setWithdrawing(true)
        try {
            const res = await fetch(`${API}/pool/withdraw`, {
                method: 'POST',
                credentials: 'include',
                headers: authHeaders(),
                body: JSON.stringify({ amount: centavos }),
            })
            if (!res.ok) throw new Error(await res.text() || 'Unable to withdraw')
            const data = await res.json() as { message: string }
            toast.success(data.message)
            onChanged()
            return true
        } catch (err) {
            toast.error(err instanceof Error ? err.message : 'Unable to withdraw')
            return false
        } finally {
            setWithdrawing(false)
        }
    }, [authHeaders, onChanged, toast])

    return { confirmDeposit, confirming, withdraw, withdrawing }
}
