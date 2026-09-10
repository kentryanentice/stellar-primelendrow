import { useCallback, useEffect, useState } from 'react'
import { useSession } from '../../providers/useSession'
import { useToast } from '../../providers/useToast'
import { submitVaultMovement, type SeizureQuote, type VaultAction } from '../Lending/stellarAdmin'

const API = import.meta.env.VITE_API_URL ?? ''

export type AdminInstallment = {
    installment: number
    due_at: number
    principal_due: number
    interest_due: number
    principal_paid: number
    interest_paid: number
    status: 'scheduled' | 'paid' | 'late' | 'defaulted'
}

export type AdminCollateral = {
    wallet_address: string
    required_stroops: number
    locked_stroops: number
    status: 'pending' | 'locked' | 'released' | 'seized'
}

/** One settled step of the ARCHITECTURE §5.9 waterfall. */
export type AdminRecovery = {
    step: number
    source: 'borrower_deposit' | 'borrower_xlm' | 'guarantor_deposit' | 'reserve_fund'
    username: string | null
    amount: number
    /** How much of this step a settlement has given back. Only the guarantor
     *  and reserve steps are refundable; the borrower's own seized assets are
     *  not returned, having paid the borrower's own debt. */
    refunded: number
    stroops: number | null
    created_at: number
}

/** One thing that happened to a loan. On-chain rows carry the hash that
 *  proves them; the peso rows carry PayPal's reference where there is one. */
export type AdminMovement = {
    kind:
        | 'collateral_lock'
        | 'disbursed'
        | 'payout'
        | 'payment'
        | 'collateral_mark_repaid'
        | 'collateral_release'
        | 'collateral_mark_defaulted'
        | 'collateral_seize'
    at: number
    status: string
    amount: number | null
    stroops: number | null
    tx_hash: string | null
    reference: string | null
}

export type AdminLoan = {
    id: string
    borrower: string
    product: string
    principal: number
    principal_outstanding: number
    rate_bps: number
    term_months: number
    /** `reconciling` = defaulted but reopened so the borrower can settle;
     *  `reconciled` = they did, and their standing was restored (033). */
    status: 'pending' | 'active' | 'closed' | 'defaulted' | 'reconciling' | 'reconciled' | 'declined' | 'cancelled'
    /** What a settling borrower still owes: the money guarantors and the
     *  reserve fund are short after the default, net of what a partial
     *  settlement has already given back. Deliberately NOT the unpaid
     *  schedule — the waterfall settled this debt on the spot, so charging
     *  every remaining month would bill for credit nobody extended and for
     *  the borrower's own seized deposit twice (033). */
    arrears: number
    disbursed_at: number | null
    defaulted_at: number | null
    closed_at: number | null
    schedule: AdminInstallment[]
    collateral: AdminCollateral | null
    recoveries: AdminRecovery[]
    movements: AdminMovement[]
}

export type QueuedAction = {
    id: number
    loan_id: string
    action: VaultAction
    borrower: string
    wallet_address: string
    locked_stroops: number
    quote_php_per_xlm_centavos: number | null
    created_at: number
}

type LoansPage = {
    items: AdminLoan[]
    total: number
    page: number
    page_size: number
    total_pages: number
}

type ActionsPayload = {
    actions: QueuedAction[]
    contract_id: string | null
    treasury: string | null
}

type PreparedAction = {
    id: number
    loan_id: string
    action: VaultAction
    contract_id: string
    treasury: string | null
    quote: SeizureQuote | null
}

export type LoanFilter = 'open' | 'defaulted' | 'settling' | 'all'

/**
 * The operator's lending console: the loan book, declaring a default, and
 * draining the vault's outbox.
 *
 * The outbox is the part worth understanding. `collateral_actions` has always
 * been a queue of movements only the vault admin key can make, and until now
 * nothing drained it. Signing happens HERE, in the operator's wallet, never on
 * the server — so this hook's job for each queued movement is three calls:
 *
 *   prepare (engine pins the price and destination)
 *     -> submitVaultMovement (Freighter signs, the network settles)
 *       -> confirm (engine verifies the hash on Horizon and moves the books)
 *
 * The engine decides every number in step one; nothing an operator can type
 * reaches the contract. `use`-prefixed per this repo's React Compiler
 * requirement.
 */
export default function useAdminLending() {
    const { csrfToken } = useSession()
    const toast = useToast()

    const [loans, setLoans] = useState<AdminLoan[]>([])
    const [filter, setFilter] = useState<LoanFilter>('open')
    const [page, setPage] = useState(1)
    const [total, setTotal] = useState(0)
    const [totalPages, setTotalPages] = useState(1)
    const [loading, setLoading] = useState(true)
    const [error, setError] = useState(false)

    const [actions, setActions] = useState<QueuedAction[]>([])
    const [contractId, setContractId] = useState<string | null>(null)
    /** The movement currently being signed, so only its button spins. */
    const [busyAction, setBusyAction] = useState<number | null>(null)
    const [defaultingId, setDefaultingId] = useState<string | null>(null)
    /** The loan a reopen-or-settle call is in flight for (033). Separate from
     *  `defaultingId` so the two can't disable each other's buttons. */
    const [reconcilingId, setReconcilingId] = useState<string | null>(null)

    const authHeaders = useCallback((): HeadersInit => ({
        'Content-Type': 'application/json',
        ...(csrfToken ? { 'x-csrf-token': csrfToken } : {}),
    }), [csrfToken])

    const post = useCallback(async <T,>(path: string, body: unknown): Promise<T> => {
        const res = await fetch(`${API}${path}`, {
            method: 'POST',
            credentials: 'include',
            headers: authHeaders(),
            body: JSON.stringify(body),
        })
        if (!res.ok) throw new Error(await res.text() || 'Request failed')
        return await res.json() as T
    }, [authHeaders])

    const loadLoans = useCallback(async (targetPage: number, targetFilter: LoanFilter) => {
        setLoading(true)
        setError(false)
        try {
            const data = await post<LoansPage>('/lending/admin/loans', { page: targetPage, filter: targetFilter })
            setLoans(data.items)
            setPage(data.page)
            setTotal(data.total)
            setTotalPages(data.total_pages)
        } catch {
            setError(true)
        } finally {
            setLoading(false)
        }
    }, [post])

    const loadActions = useCallback(async () => {
        try {
            const data = await post<ActionsPayload>('/lending/admin/actions', {})
            setActions(data.actions)
            setContractId(data.contract_id)
        } catch {
            setActions([])
        }
    }, [post])

    const refresh = useCallback(async () => {
        await Promise.all([loadLoans(page, filter), loadActions()])
    }, [loadLoans, loadActions, page, filter])

    useEffect(() => { void loadLoans(1, filter) }, [loadLoans, filter])
    useEffect(() => { void loadActions() }, [loadActions])

    /** Declares a default. The engine takes the borrower's own deposits at
     *  once and then waits for the coins — the response says which. */
    const declareDefault = useCallback(async (loanId: string, reason: string) => {
        setDefaultingId(loanId)
        try {
            const data = await post<{ message: string }>('/lending/admin/loans/default', {
                loan_id: loanId,
                reason,
            })
            toast.success(data.message)
            await Promise.all([loadLoans(page, filter), loadActions()])
            return true
        } catch (err) {
            toast.error(err instanceof Error ? err.message : 'Unable to default this loan')
            return false
        } finally {
            setDefaultingId(null)
        }
    }, [post, toast, loadLoans, loadActions, page, filter])

    /**
     * Reopens a defaulted loan so the borrower can pay their arrears (033).
     *
     * Moves no money — it makes the loan payable again and puts the arrears on
     * the borrower's Pay page. The settlement itself arrives through the
     * ordinary rail, verified by the provider, which is why an admin can offer
     * a borrower this second chance without being able to fake that they took
     * it.
     */
    const reopenForSettlement = useCallback(async (loanId: string, reason: string) => {
        setReconcilingId(loanId)
        try {
            const data = await post<{ message: string; arrears: number }>(
                '/lending/admin/loans/reopen',
                { loan_id: loanId, reason },
            )
            toast.success(data.message)
            await loadLoans(page, filter)
            return true
        } catch (err) {
            toast.error(err instanceof Error ? err.message : 'Unable to reopen this loan')
            return false
        } finally {
            setReconcilingId(null)
        }
    }, [post, toast, loadLoans, page, filter])

    /**
     * Accepts a reopened loan as settled: restores the credit penalty and
     * queues the on-chain outcome, so the borrower can apply again.
     *
     * The engine refuses while any arrears remain, so this button is an
     * acceptance of a fact, not an assertion of one.
     */
    const markSettled = useCallback(async (loanId: string, reason: string) => {
        setReconcilingId(loanId)
        try {
            const data = await post<{ message: string; note: string | null }>(
                '/lending/admin/loans/reconcile',
                { loan_id: loanId, reason },
            )
            toast.success(data.message)
            // Seized coins can't be returned from here; the engine says so
            // rather than reporting a clean settlement, and so does the UI.
            if (data.note) toast.error(data.note)
            await Promise.all([loadLoans(page, filter), loadActions()])
            return true
        } catch (err) {
            toast.error(err instanceof Error ? err.message : 'Unable to mark this loan settled')
            return false
        } finally {
            setReconcilingId(null)
        }
    }, [post, toast, loadLoans, loadActions, page, filter])

    /**
     * Records a movement that is ALREADY on-chain, from its transaction hash.
     *
     * Needed because the contract settles an outcome once: if a movement
     * succeeded on the network but the engine failed to record it — a Horizon
     * hiccup, a closed tab — signing again is refused on-chain ("positions
     * settle once") and the queued row would be stuck forever. The engine
     * still verifies the hash against Horizon and checks it called this
     * movement's own entry point, so this is a recovery path, not a bypass.
     */
    const confirmHash = useCallback(async (actionId: number, txHash: string) => {
        setBusyAction(actionId)
        try {
            const done = await post<{ message: string }>('/lending/admin/actions/confirm', {
                action_id: actionId,
                tx_hash: txHash.trim(),
            })
            toast.success(done.message)
            await Promise.all([loadLoans(page, filter), loadActions()])
            return true
        } catch (err) {
            toast.error(err instanceof Error ? err.message : 'Unable to record that transaction')
            return false
        } finally {
            setBusyAction(null)
        }
    }, [post, toast, loadLoans, loadActions, page, filter])

    /** Runs one queued movement all the way through: prepare, sign, confirm. */
    const signAction = useCallback(async (action: QueuedAction) => {
        setBusyAction(action.id)
        try {
            const prepared = await post<PreparedAction>('/lending/admin/actions/prepare', {
                action_id: action.id,
            })

            const submitted = await submitVaultMovement({
                contractId: prepared.contract_id,
                action: prepared.action,
                loanId: prepared.loan_id,
                treasury: prepared.treasury,
                quote: prepared.quote,
            })
            if ('error' in submitted) {
                toast.error(submitted.error)
                return false
            }

            const done = await post<{ message: string }>('/lending/admin/actions/confirm', {
                action_id: action.id,
                tx_hash: submitted.txHash,
            })
            toast.success(done.message)
            await Promise.all([loadLoans(page, filter), loadActions()])
            return true
        } catch (err) {
            toast.error(err instanceof Error ? err.message : 'Unable to record that movement')
            return false
        } finally {
            setBusyAction(null)
        }
    }, [post, toast, loadLoans, loadActions, page, filter])

    return {
        loans, page, total, totalPages, loading, error,
        filter, setFilter,
        goToPage: (target: number) => void loadLoans(target, filter),
        refresh,
        actions, contractId,
        declareDefault, defaultingId,
        reopenForSettlement, markSettled, reconcilingId,
        signAction, confirmHash, busyAction,
    }
}
