import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { CalendarClock, ChevronDown, ChevronUp } from 'lucide-react'
import { formatDate, pesos } from '../../functions/Lending/money'
import { PRODUCT_LABEL, type Loan } from '../../functions/Lending/types'
import type { PaymentTotals } from '../../functions/Lending/usePayments'
import { nextInstallment } from '../../functions/Dashboard/overview'
import { NextPaymentBodySkeleton } from './DashboardSkeleton'

type Props = {
    /** The active or settling loan, if any. */
    loan: Loan | null
    pendingLoan: Loan | null
    loading: boolean
    error: boolean
    repaid: PaymentTotals
    paymentCount: number
}

/**
 * What's due next on the one open loan, and a way to pay it. Mirrors the Pay
 * page's rules rather than inventing its own: a settling loan owes its
 * arrears (never the schedule), and the pay button only appears when there's
 * something to charge — otherwise a settled loan would keep inviting payment.
 */
function NextPaymentCard({ loan, pendingLoan, loading, error, repaid, paymentCount }: Props) {
    const navigate = useNavigate()
    const [scheduleOpen, setScheduleOpen] = useState(false)

    const settling = loan?.status === 'reconciling'
    const next = loan && !settling ? nextInstallment(loan) : null
    const payNow = loan ? (settling ? loan.arrears : (next?.total ?? 0)) : 0
    const paidRows = loan ? loan.schedule.filter(row => row.status === 'paid').length : 0

    const pill = !loan ? null
        : settling ? { label: 'Settling', cls: 'is-warn' }
        : next?.late ? { label: 'Late', cls: 'is-warn' }
        : { label: 'Active', cls: 'is-progress' }

    return (
        <section className='lending-card'>
            <div className='lending-card-head'>
                <span className='lending-card-icon is-accent'><CalendarClock /></span>
                <h2>{settling ? 'Settle your loan' : 'Next payment due'}</h2>
                {pill && <span className={`lending-tx-status ${pill.cls}`}>{pill.label}</span>}
            </div>

            {loading ? (
                <NextPaymentBodySkeleton />
            ) : error ? (
                <p className='lending-muted'>Couldn’t load your loans. Please try again later.</p>
            ) : !loan ? (
                <div className='dash-due-row'>
                    <p className='lending-muted'>
                        {pendingLoan ? (
                            <>
                                Your <b>{pesos(pendingLoan.principal)}</b> {PRODUCT_LABEL[pendingLoan.product].toLowerCase()} is
                                waiting on setup — nothing is due until it’s disbursed.
                            </>
                        ) : (
                            <><b>You’re all paid up.</b> No active loans to repay right now.</>
                        )}
                    </p>
                    <button type='button' className='lending-btn' onClick={() => navigate('/borrow')}>
                        {pendingLoan ? 'Finish setup' : 'Apply for a loan'}
                    </button>
                </div>
            ) : (
                <>
                    <div className='dash-due-row'>
                        <div className='dash-due-figure'>
                            <span className='dash-due-amount'>{pesos(payNow)}</span>
                            <span className='lending-muted'>
                                {settling
                                    ? 'Owed to settle this defaulted loan · you can pay it in parts'
                                    : next
                                        ? `${next.late ? 'Overdue since' : 'Due'} ${formatDate(next.dueAt)} · installment ${next.installment} of ${loan.schedule.length} · ${PRODUCT_LABEL[loan.product].toLowerCase()}`
                                        : 'Nothing due on this loan right now'}
                            </span>
                        </div>
                        <div className='dash-due-actions'>
                            {payNow > 0 && (
                                <button type='button' className='lending-btn-primary' onClick={() => navigate('/pay')}>Pay this loan</button>
                            )}
                            {loan.schedule.length > 0 && (
                                <button
                                    type='button'
                                    className='lending-btn'
                                    aria-expanded={scheduleOpen}
                                    onClick={() => setScheduleOpen(open => !open)}
                                >
                                    {scheduleOpen ? 'Hide schedule' : 'View schedule'} {scheduleOpen ? <ChevronUp /> : <ChevronDown />}
                                </button>
                            )}
                        </div>
                    </div>

                    {scheduleOpen && (
                        <div className='lending-schedule-scroll'>
                            <table className='lending-schedule'>
                                <thead>
                                    <tr><th>#</th><th>Due</th><th>Amount</th><th>Status</th></tr>
                                </thead>
                                <tbody>
                                    {loan.schedule.map(row => (
                                        <tr key={row.installment} className={row.status === 'paid' ? 'is-paid' : undefined}>
                                            <td>{row.installment}</td>
                                            <td>{formatDate(row.due_at)}</td>
                                            <td>{pesos(row.principal_due + row.interest_due)}</td>
                                            <td>{next?.installment === row.installment ? 'due next' : row.status}</td>
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                        </div>
                    )}

                    {loan.schedule.length > 0 && (
                        <div className='dash-progress'>
                            <div
                                className='dash-progress-track'
                                role='progressbar'
                                aria-label='Installments paid'
                                aria-valuemin={0}
                                aria-valuemax={loan.schedule.length}
                                aria-valuenow={paidRows}
                            >
                                <div className='dash-progress-fill' style={{ width: `${(paidRows / loan.schedule.length) * 100}%` }} />
                            </div>
                            <div className='dash-progress-meta'>
                                <span>{pesos(loan.principal_outstanding)} outstanding principal · {paidRows} of {loan.schedule.length} paid</span>
                                {paymentCount > 0 && (
                                    <span>{pesos(repaid.amount_received)} repaid to date · {paymentCount} {paymentCount === 1 ? 'payment' : 'payments'}</span>
                                )}
                            </div>
                        </div>
                    )}
                </>
            )}
        </section>
    )
}

export default NextPaymentCard
