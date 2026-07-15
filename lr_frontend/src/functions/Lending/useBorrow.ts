import { useCallback, useEffect, useRef, useState } from 'react'
import { useSession } from '../../providers/useSession'
import { useToast } from '../../providers/useToast'
import { lockAndConfirmCollateral } from './stellarLock'
import type { ApplyResponse, Product, QuoteResponse } from './types'

const API = import.meta.env.VITE_API_URL ?? ''
/** How long after the last keystroke before asking the engine for a quote. */
const QUOTE_DEBOUNCE_MS = 350

export type GuarantorAsk = { username: string; pledge_amount: number }

/**
 * The borrow flow: live engine quotes while the form is edited, the apply
 * POST (intent only — product, amount, term, invitees; the engine prices and
 * gates), and the XLM lock-then-confirm continuation for collateral loans.
 * `use`-prefixed per this repo's React Compiler requirement.
 */
export default function useBorrow(onChanged: () => void) {
    const { csrfToken } = useSession()
    const toast = useToast()

    const [quote, setQuote] = useState<QuoteResponse | null>(null)
    const [quoting, setQuoting] = useState(false)
    const [applying, setApplying] = useState(false)
    /** A just-applied XLM loan waiting on its on-chain lock. */
    const [pendingLock, setPendingLock] = useState<ApplyResponse | null>(null)
    const [locking, setLocking] = useState(false)
    const debounceTimer = useRef<number | undefined>(undefined)

    const authHeaders = useCallback((): HeadersInit => ({
        'Content-Type': 'application/json',
        ...(csrfToken ? { 'x-csrf-token': csrfToken } : {}),
    }), [csrfToken])

    useEffect(() => () => {
        if (debounceTimer.current) window.clearTimeout(debounceTimer.current)
    }, [])

    /** Debounced: the engine's numbers replace whatever the screen showed. */
    const requestQuote = useCallback((product: Product, amountCentavos: number | null, termMonths: number) => {
        if (debounceTimer.current) window.clearTimeout(debounceTimer.current)
        debounceTimer.current = window.setTimeout(async () => {
            setQuoting(true)
            try {
                const params = new URLSearchParams({ product, term_months: String(termMonths) })
                if (amountCentavos) params.set('amount', String(amountCentavos))
                const res = await fetch(`${API}/loans/quote?${params}`, { credentials: 'include' })
                if (!res.ok) throw new Error()
                setQuote(await res.json() as QuoteResponse)
            } catch {
                setQuote(null)
            } finally {
                setQuoting(false)
            }
        }, QUOTE_DEBOUNCE_MS)
    }, [])

    const apply = useCallback(async (input: {
        product: Product
        amount: number
        term_months: number
        wallet_id?: string
        guarantors?: GuarantorAsk[]
    }) => {
        setApplying(true)
        try {
            const res = await fetch(`${API}/loans/apply`, {
                method: 'POST',
                credentials: 'include',
                headers: authHeaders(),
                body: JSON.stringify(input),
            })
            if (!res.ok) throw new Error(await res.text() || 'Unable to submit your application')
            const data = await res.json() as ApplyResponse
            toast.success(data.message)
            if (input.product === 'xlm_collateral') {
                // The loan is pending until the wallet locks the collateral.
                setPendingLock(data)
            }
            onChanged()
            return data
        } catch (err) {
            toast.error(err instanceof Error ? err.message : 'Unable to submit your application')
            return null
        } finally {
            setApplying(false)
        }
    }, [authHeaders, onChanged, toast])

    /** Lock the engine-required stroops on-chain, then hand the tx hash back
     *  for verification — the engine only believes Horizon. */
    const lockAndConfirm = useCallback(async (walletAddress: string) => {
        if (!pendingLock?.required_stroops || !pendingLock.collateral_contract) return
        setLocking(true)
        try {
            const result = await lockAndConfirmCollateral({
                contractId: pendingLock.collateral_contract,
                walletAddress,
                loanId: pendingLock.loan_id,
                stroops: pendingLock.required_stroops,
                csrfToken,
            })
            if ('error' in result) {
                toast.error(result.error)
                return
            }
            toast.success(result.message)
            setPendingLock(null)
            onChanged()
        } finally {
            setLocking(false)
        }
    }, [csrfToken, onChanged, pendingLock, toast])

    return {
        quote, quoting, requestQuote,
        apply, applying,
        pendingLock, setPendingLock, lockAndConfirm, locking,
    }
}
