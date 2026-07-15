import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { ClipboardList, ChevronDown, ChevronUp, ChevronLeft, ChevronRight, AlertTriangle, CreditCard } from 'lucide-react'
import type useLoanHistory from '../../functions/Lending/useLoanHistory'
import { useSession } from '../../providers/useSession'
import { useToast } from '../../providers/useToast'
import { lockAndConfirmCollateral } from '../../functions/Lending/stellarLock'
import { formatDate, pesos, rate, xlm } from '../../functions/Lending/money'
import { PRODUCT_LABEL, type Loan, type PoolResponse } from '../../functions/Lending/types'

const STATUS_CLS: Record<Loan['status'], string> = {
    pending: 'is-pending',
    active: 'is-active',
    closed: 'is-closed',
    defaulted: 'is-defaulted',
    declined: 'is-declined',
    cancelled: 'is-declined',
}

/**
 * The caller's loans, one page at a time (POST /loans/history) — status,
 * pinned schedule, collateral health for XLM positions, and the resume path
 * for a pending lock that was interrupted. Read-only history; actually
 * paying an installment happens on the Pay page (the "Pay" button below
 * just routes there).
 */
function LoanHistoryCard({ data, history, onChanged }: {
    data: PoolResponse
    history: ReturnType<typeof useLoanHistory>
    onChanged: () => void
}) {
    const { loans, page, total, totalPages, loading, error, refresh, goToPage } = history
    const { csrfToken } = useSession()
    const toast = useToast()
    const navigate = useNavigate()
    const [openId, setOpenId] = useState<string | null>(null)
    const [lockingId, setLockingId] = useState<string | null>(null)

    const resumeLock = async (loan: Loan) => {
        if (!loan.collateral || !data.params.collateral_contract) return
        setLockingId(loan.id)
        try {
            const result = await lockAndConfirmCollateral({
                contractId: data.params.collateral_contract,
                walletAddress: loan.collateral.wallet_address,
                loanId: loan.id,
                stroops: loan.collateral.required_stroops,
                csrfToken,
            })
            if ('error' in result) {
                toast.error(result.error)
                return
            }
            toast.success(result.message)
            await refresh()
            onChanged()
        } finally {
            setLockingId(null)
        }
    }

    return (
        <section className='lending-card lending-card-loans'>
            <div className='lending-card-head'>
                <span className='lending-card-icon is-accent'><ClipboardList /></span>
                <h2>Your loans</h2>
            </div>

            {loading ? (
                <p className='lending-muted'>Loading your loans…</p>
            ) : error ? (
                <p className='lending-muted'>Couldn’t load your loans. Please try again later.</p>
            ) : total === 0 ? (
                <p className='lending-muted'>No loans yet — when you borrow, the full repayment schedule shows up here.</p>
            ) : (
                <>
                    <ul className='lending-loans'>
                        {loans.map(loan => {
                            const open = openId === loan.id
                            const awaitingLock = loan.status === 'pending' && loan.collateral?.status === 'pending'
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
                                            <span>{PRODUCT_LABEL[loan.product]} · {rate(loan.rate_bps)} · {loan.term_months} mo</span>
                                        </div>
                                        <span className={`lending-loan-status ${STATUS_CLS[loan.status]}`}>{loan.status}</span>
                                        {open ? <ChevronUp aria-hidden='true' /> : <ChevronDown aria-hidden='true' />}
                                    </button>

                                    {open && (
                                        <div className='lending-loan-detail'>
                                            {loan.status === 'active' && (
                                                <p className='lending-muted'>
                                                    Outstanding principal: <b>{pesos(loan.principal_outstanding)}</b>
                                                    {loan.disbursed_at && <> · disbursed {formatDate(loan.disbursed_at)}</>}
                                                </p>
                                            )}

                                            {loan.collateral && (
                                                <p className={`lending-muted${loan.collateral.liquidatable ? ' lending-liquidation' : ''}`}>
                                                    {loan.collateral.liquidatable && <AlertTriangle />}
                                                    Collateral: {xlm(loan.collateral.locked_stroops || loan.collateral.required_stroops)} ({loan.collateral.status})
                                                    {loan.collateral.health_pct !== null && <> · health {loan.collateral.health_pct}%</>}
                                                    {loan.collateral.liquidatable && <> — below the liquidation threshold, top-up is not supported: repay to protect it</>}
                                                </p>
                                            )}

                                            {loan.guarantors.length > 0 && (
                                                <p className='lending-muted'>
                                                    Guarantors: {loan.guarantors.map(g => `${g.username} (${pesos(g.pledge_amount)}, ${g.status})`).join(', ')}
                                                </p>
                                            )}

                                            {awaitingLock && (
                                                <button
                                                    type='button'
                                                    className='lending-btn-primary'
                                                    disabled={lockingId === loan.id || !data.params.collateral_contract}
                                                    onClick={() => resumeLock(loan)}
                                                >
                                                    {lockingId === loan.id ? 'Locking on-chain…' : `Lock ${xlm(loan.collateral?.required_stroops ?? 0)} with Freighter`}
                                                </button>
                                            )}

                                            {loan.schedule.length > 0 && (
                                                <div className='lending-schedule-scroll'>
                                                    <table className='lending-schedule'>
                                                        <thead>
                                                            <tr><th>#</th><th>Due</th><th>Principal</th><th>Interest</th><th>Status</th></tr>
                                                        </thead>
                                                        <tbody>
                                                            {loan.schedule.map(row => (
                                                                <tr key={row.installment} className={row.status === 'paid' ? 'is-paid' : undefined}>
                                                                    <td>{row.installment}</td>
                                                                    <td>{formatDate(row.due_at)}</td>
                                                                    <td>{pesos(row.principal_due)}</td>
                                                                    <td>{pesos(row.interest_due)}</td>
                                                                    <td>{row.status}</td>
                                                                </tr>
                                                            ))}
                                                        </tbody>
                                                    </table>
                                                </div>
                                            )}

                                            {loan.status === 'active' && (
                                                <button
                                                    type='button'
                                                    className='lending-btn-primary lending-loan-pay'
                                                    onClick={() => navigate('/pay')}
                                                >
                                                    <CreditCard aria-hidden='true' /> Pay this loan
                                                </button>
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
        </section>
    )
}

export default LoanHistoryCard
