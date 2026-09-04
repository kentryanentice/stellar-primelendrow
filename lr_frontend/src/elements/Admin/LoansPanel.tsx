import { useState } from 'react'
import { AlertTriangle, ChevronDown, ChevronLeft, ChevronRight, ChevronUp, ExternalLink, PenLine, ShieldAlert } from 'lucide-react'
import type useAdminLending from '../../functions/Admin/useAdminLending'
import type { AdminInstallment, AdminLoan, LoanFilter } from '../../functions/Admin/useAdminLending'
import type { VaultAction } from '../../functions/Lending/stellarAdmin'
import { formatDate, pesos, rate, xlm } from '../../functions/Lending/money'
import { shortId, txLink } from '../../functions/Lending/explorer'

const FILTERS: { key: LoanFilter; label: string }[] = [
    { key: 'open', label: 'Open' },
    { key: 'defaulted', label: 'Defaulted' },
    { key: 'all', label: 'All' },
]

const STATUS_CLS: Record<AdminLoan['status'], string> = {
    pending: 'is-pending',
    active: 'is-active',
    closed: 'is-closed',
    defaulted: 'is-defaulted',
    declined: 'is-declined',
    cancelled: 'is-declined',
}

/** What each queued movement is about to do, in the operator's language. */
const ACTION_LABEL: Record<VaultAction, string> = {
    mark_repaid: 'Record repayment on-chain',
    release: 'Release collateral to the borrower',
    mark_defaulted: 'Record default on-chain',
    seize: 'Seize collateral to the treasury',
}

/** One line of a loan's life, in the order it happened. */
const MOVEMENT_LABEL: Record<string, string> = {
    collateral_lock: 'Collateral locked into the vault',
    disbursed: 'Loan disbursed',
    payout: 'Proceeds sent to PayPal',
    payment: 'Repayment received',
    collateral_mark_repaid: 'Repayment recorded on-chain',
    collateral_release: 'Collateral released to the borrower',
    collateral_mark_defaulted: 'Default recorded on-chain',
    collateral_seize: 'Collateral seized to the treasury',
}

/** Anything the chain or the rail hasn't settled yet reads as in-progress. */
const SETTLED = new Set(['confirmed', 'completed', 'paid'])

const RECOVERY_LABEL: Record<string, string> = {
    borrower_deposit: 'Borrower deposit',
    borrower_xlm: 'Borrower XLM',
    guarantor_deposit: 'Guarantor deposit',
    reserve_fund: 'Reserve fund',
}

/** The unpaid remainder of one installment — what defaulting would call in. */
const outstandingOn = (row: AdminInstallment) =>
    (row.principal_due - row.principal_paid) + (row.interest_due - row.interest_paid)

/**
 * The operator's lending console.
 *
 * Two halves, in the order they happen. The loan book, where a default is
 * declared against a specific installment — the row makes it concrete: this is
 * the payment that was missed. Then the vault outbox, where the movements that
 * default (or a repayment) queued get signed with the admin key.
 *
 * The Sign buttons are the only place in the product where the vault's
 * admin-only entry points are called. They go through the operator's own
 * wallet: the engine prepares the parameters and verifies the result, but it
 * never holds the key, so nothing on the server can move coins out of the
 * vault on its own.
 */
function LoansPanel({ lending }: { lending: ReturnType<typeof useAdminLending> }) {
    const {
        loans, page, total, totalPages, loading, error,
        filter, setFilter, goToPage,
        actions, contractId,
        declareDefault, defaultingId,
        signAction, confirmHash, busyAction,
    } = lending

    const [openId, setOpenId] = useState<string | null>(null)
    /** The movement whose "already signed" hash field is open, and its value. */
    const [pasting, setPasting] = useState<number | null>(null)
    const [hash, setHash] = useState('')
    /** The loan a default is being confirmed for, and the reason typed for it. */
    const [confirming, setConfirming] = useState<{ loan: AdminLoan; installment: number } | null>(null)
    const [reason, setReason] = useState('')

    return (
        <section className='admin-loans'>
            <div className='admin-loans-head'>
                <h2>Loan book</h2>
                <div className='admin-loans-filters'>
                    {FILTERS.map(f => (
                        <button
                            key={f.key}
                            type='button'
                            className={`lending-tab${filter === f.key ? ' is-active' : ''}`}
                            onClick={() => setFilter(f.key)}
                        >
                            {f.label}
                        </button>
                    ))}
                </div>
            </div>

            {/* ---- the vault outbox ------------------------------------- */}
            {actions.length > 0 && (
                <div className='admin-outbox'>
                    <div className='admin-outbox-head'>
                        <span className='lending-card-icon is-accent'><ShieldAlert /></span>
                        <h3>Vault movements awaiting your key</h3>
                    </div>
                    <p className='lending-muted'>
                        Signed in your own wallet, never on the server. The contract enforces the order —
                        a release is refused until the repayment is recorded, a seizure until the default is.
                    </p>
                    {!contractId && (
                        <p className='lending-field-error'>
                            No vault contract is configured on this deployment, so none of these can be executed.
                        </p>
                    )}
                    <ul className='admin-outbox-list'>
                        {actions.map((action, i) => {
                            // The contract refuses `release` before a recorded
                            // repayment and `seize` before a recorded default,
                            // so a movement whose predecessor on the same loan
                            // is still queued cannot succeed yet. The list is
                            // in queue order, which is execution order.
                            const blocked = actions.slice(0, i).some(earlier => earlier.loan_id === action.loan_id)
                            return (
                            <li key={action.id} className='admin-outbox-row'>
                                <div className='admin-outbox-what'>
                                    <b>{ACTION_LABEL[action.action]}</b>
                                    <span className='lending-muted'>
                                        {action.borrower} · {xlm(action.locked_stroops)} · queued {formatDate(action.created_at)}
                                        {blocked && ' · waiting on the movement above it'}
                                    </span>
                                </div>
                                <button
                                    type='button'
                                    className='lending-btn-primary'
                                    disabled={busyAction !== null || !contractId || blocked}
                                    title={blocked ? 'Record the outcome above this one on-chain first' : undefined}
                                    onClick={() => void signAction(action)}
                                >
                                    <PenLine aria-hidden='true' />
                                    {busyAction === action.id ? 'Signing…' : 'Sign'}
                                </button>

                                {/* The recovery path. The contract settles an
                                    outcome once, so a movement that went
                                    through on-chain but wasn't recorded here
                                    cannot be signed again — it has to be
                                    recorded from its hash instead. The engine
                                    still verifies it on Horizon. */}
                                <button
                                    type='button'
                                    className='admin-outbox-alt'
                                    onClick={() => {
                                        setPasting(pasting === action.id ? null : action.id)
                                        setHash('')
                                    }}
                                >
                                    {pasting === action.id ? 'Cancel' : 'Already signed?'}
                                </button>

                                {pasting === action.id && (
                                    <div className='admin-outbox-paste'>
                                        <input
                                            className='lending-input'
                                            placeholder='Transaction hash (64 hex characters)'
                                            value={hash}
                                            onChange={e => setHash(e.target.value)}
                                            aria-label='Transaction hash'
                                        />
                                        <button
                                            type='button'
                                            className='lending-btn'
                                            disabled={busyAction !== null || !/^[0-9a-fA-F]{64}$/.test(hash.trim())}
                                            onClick={async () => {
                                                if (await confirmHash(action.id, hash)) setPasting(null)
                                            }}
                                        >
                                            {busyAction === action.id ? 'Recording…' : 'Record'}
                                        </button>
                                    </div>
                                )}
                            </li>
                            )
                        })}
                    </ul>
                </div>
            )}


            {/* ---- the loan book ---------------------------------------- */}
            {loading ? (
                <p className='lending-muted'>Loading the loan book…</p>
            ) : error ? (
                <p className='lending-muted'>Couldn’t load the loan book. Please try again.</p>
            ) : total === 0 ? (
                <p className='lending-muted'>No loans match this filter.</p>
            ) : (
                <>
                    <ul className='lending-loans'>
                        {loans.map(loan => {
                            const open = openId === loan.id
                            return (
                                <li key={loan.id} className='lending-loan'>
                                    <button
                                        type='button'
                                        className='lending-loan-summary'
                                        aria-expanded={open}
                                        onClick={() => setOpenId(open ? null : loan.id)}
                                    >
                                        <div className='lending-loan-title'>
                                            <b>{pesos(loan.principal)}</b>
                                            <span>
                                                {loan.borrower} · {loan.product} · {rate(loan.rate_bps)} ·{' '}
                                                {pesos(loan.principal_outstanding)} outstanding
                                            </span>
                                        </div>
                                        <span className={`lending-loan-status ${STATUS_CLS[loan.status]}`}>{loan.status}</span>
                                        {open ? <ChevronUp aria-hidden='true' /> : <ChevronDown aria-hidden='true' />}
                                    </button>

                                    {open && (
                                        <div className='lending-loan-detail'>
                                            {loan.collateral && (
                                                <p className='lending-muted'>
                                                    Collateral: {xlm(loan.collateral.locked_stroops || loan.collateral.required_stroops)}
                                                    {' '}({loan.collateral.status}) from {loan.collateral.wallet_address.slice(0, 5)}…
                                                    {loan.collateral.wallet_address.slice(-4)}
                                                </p>
                                            )}

                                            {loan.defaulted_at !== null && (
                                                <p className='lending-muted lending-liquidation'>
                                                    <AlertTriangle />
                                                    Defaulted {formatDate(loan.defaulted_at)}
                                                    {loan.principal_outstanding > 0
                                                        ? ` — ${pesos(loan.principal_outstanding)} still uncovered`
                                                        : ' — recovery settled'}
                                                </p>
                                            )}

                                            {loan.recoveries.length > 0 && (
                                                <div className='admin-recoveries'>
                                                    <span className='lending-stat-label'>Recovery waterfall</span>
                                                    {loan.recoveries.map((step, i) => (
                                                        <p key={i} className='lending-muted'>
                                                            <b>{step.step}.</b> {RECOVERY_LABEL[step.source] ?? step.source}
                                                            {step.username && ` (${step.username})`} — {pesos(step.amount)}
                                                            {step.stroops !== null && ` from ${xlm(step.stroops)}`}
                                                        </p>
                                                    ))}
                                                </div>
                                            )}

                                            {/* The loan's life, oldest first. Every on-chain row
                                                carries the transaction that proves it, so a
                                                reviewer can check the claim against the network
                                                instead of against this screen. */}
                                            {loan.movements.length > 0 && (
                                                <div className='admin-movements'>
                                                    <span className='lending-stat-label'>Transaction record</span>
                                                    {loan.movements.map((move, i) => (
                                                        <div key={i} className='admin-movement'>
                                                            <div className='admin-movement-what'>
                                                                <b>{MOVEMENT_LABEL[move.kind] ?? move.kind}</b>
                                                                <span className='lending-muted'>
                                                                    {formatDate(move.at)}
                                                                    {move.stroops !== null && ` · ${xlm(move.stroops)}`}
                                                                    {move.amount !== null && ` · ${pesos(move.amount)}`}
                                                                    {!SETTLED.has(move.status) && ` · ${move.status}`}
                                                                </span>
                                                            </div>
                                                            {move.tx_hash ? (
                                                                <a
                                                                    className='lending-inline-link'
                                                                    href={txLink(move.tx_hash)}
                                                                    target='_blank'
                                                                    rel='noreferrer noopener'
                                                                >
                                                                    <span>{shortId(move.tx_hash)}</span>
                                                                    <ExternalLink aria-hidden='true' />
                                                                </a>
                                                            ) : move.reference ? (
                                                                <span className='admin-movement-ref'>{shortId(move.reference)}</span>
                                                            ) : (
                                                                <span className='admin-movement-ref'>—</span>
                                                            )}
                                                        </div>
                                                    ))}
                                                </div>
                                            )}

                                            {loan.schedule.length > 0 && (
                                                <div className='lending-schedule-scroll'>
                                                    <table className='lending-schedule'>
                                                        <thead>
                                                            <tr>
                                                                <th>#</th><th>Due</th><th>Owed</th><th>Status</th><th />
                                                            </tr>
                                                        </thead>
                                                        <tbody>
                                                            {loan.schedule.map(row => (
                                                                <tr key={row.installment} className={row.status === 'paid' ? 'is-paid' : undefined}>
                                                                    <td>{row.installment}</td>
                                                                    <td>{formatDate(row.due_at)}</td>
                                                                    <td>{pesos(outstandingOn(row))}</td>
                                                                    <td>{row.status}</td>
                                                                    <td>
                                                                        {/* Only a running loan can be called, and only
                                                                            on an installment that is actually unpaid —
                                                                            the button is per-row so the operator names
                                                                            the payment that was missed. */}
                                                                        {loan.status === 'active' && row.status !== 'paid' && (
                                                                            <button
                                                                                type='button'
                                                                                className='admin-default-btn'
                                                                                disabled={defaultingId !== null}
                                                                                onClick={() => {
                                                                                    setReason(`Missed installment ${row.installment}, due ${formatDate(row.due_at)}`)
                                                                                    setConfirming({ loan, installment: row.installment })
                                                                                }}
                                                                            >
                                                                                Default
                                                                            </button>
                                                                        )}
                                                                    </td>
                                                                </tr>
                                                            ))}
                                                        </tbody>
                                                    </table>
                                                </div>
                                            )}
                                        </div>
                                    )}
                                </li>
                            )
                        })}
                    </ul>

                    {totalPages > 1 && (
                        <div className='lending-pager'>
                            <button
                                type='button'
                                className='lending-pager-btn'
                                aria-label='Previous page'
                                disabled={page <= 1}
                                onClick={() => goToPage(page - 1)}
                            >
                                <ChevronLeft />
                            </button>
                            <span className='lending-muted'>Page {page} of {totalPages}</span>
                            <button
                                type='button'
                                className='lending-pager-btn'
                                aria-label='Next page'
                                disabled={page >= totalPages}
                                onClick={() => goToPage(page + 1)}
                            >
                                <ChevronRight />
                            </button>
                        </div>
                    )}
                </>
            )}

            {/* ---- confirmation ------------------------------------------
                A default takes a member's deposit and their guarantors', and
                cannot be undone from this screen. It gets a deliberate second
                step, the same as a KYC rejection does. */}
            {confirming && (
                <div className='admin-default-modal' role='dialog' aria-modal='true' aria-label='Confirm default'>
                    <div className='admin-default-card'>
                        <h3>Default this loan?</h3>
                        <p className='lending-muted'>
                            {confirming.loan.borrower}’s {pesos(confirming.loan.principal_outstanding)} outstanding will be
                            recovered in order: their own deposit first, then their locked XLM, then their guarantors’
                            pledges. Anything left over is written off against the reserve fund. Their credit score drops
                            and the loan cannot be reopened.
                        </p>
                        {confirming.loan.collateral?.status === 'locked' && (
                            <p className='lending-muted'>
                                The XLM seizure will be queued for your key — guarantors are charged only for what the
                                coins don’t cover, so nothing reaches them until you sign it.
                            </p>
                        )}
                        <label className='lending-label' htmlFor='admin-default-reason'>Reason (recorded)</label>
                        <input
                            id='admin-default-reason'
                            className='lending-input'
                            value={reason}
                            onChange={e => setReason(e.target.value)}
                        />
                        <div className='admin-default-actions'>
                            <button
                                type='button'
                                className='lending-btn'
                                disabled={defaultingId !== null}
                                onClick={() => setConfirming(null)}
                            >
                                Cancel
                            </button>
                            <button
                                type='button'
                                className='admin-default-btn is-solid'
                                disabled={defaultingId !== null}
                                onClick={async () => {
                                    const loanId = confirming.loan.id
                                    if (await declareDefault(loanId, reason)) setConfirming(null)
                                }}
                            >
                                {defaultingId !== null ? 'Defaulting…' : 'Default this loan'}
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </section>
    )
}

export default LoansPanel
