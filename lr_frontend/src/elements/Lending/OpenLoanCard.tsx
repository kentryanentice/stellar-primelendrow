import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { ChevronDown, ChevronUp } from 'lucide-react'
import { formatDate, pesos, rate } from '../../functions/Lending/money'
import { PRODUCT_LABEL, type Loan } from '../../functions/Lending/types'

type NextInstallment = { installment: number; total: number; dueAt: number }

/** The earliest not-fully-settled installment — same derivation RepayCard.tsx
 *  (Pay page) uses, duplicated here rather than shared since this component
 *  is scoped to Borrow and the function is small and stable. */
const nextInstallment = (loan: Loan): NextInstallment | null => {
    for (const row of loan.schedule) {
        const total = (row.interest_due - row.interest_paid) + (row.principal_due - row.principal_paid)
        if (total > 0) return { installment: row.installment, total, dueAt: row.due_at }
    }
    return null
}

/**
 * The one loan (if any) currently blocking a new application — the database
 * allows at most one pending-or-active loan per borrower at a time. Quiet
 * when there isn't one: a borrower with nothing open should see the apply
 * form front and center, not an empty callout.
 *
 * A 'pending' loan (still awaiting XLM lock or guarantor acceptance) has no
 * payment schedule yet, so it gets a slimmer status line rather than fake
 * payment figures — the actual next step for it (resuming the lock) lives in
 * LoanHistoryCard below, not duplicated here.
 */
function OpenLoanCard({ loan }: { loan: Loan | null }) {
    const navigate = useNavigate()
    const [scheduleOpen, setScheduleOpen] = useState(false)
    if (!loan) return null

    if (loan.status === 'pending') {
        return (
            <div className='lending-open-loan'>
                <div className='lending-open-loan-head'>
                    <span>Open loan</span>
                    <span className='lending-loan-status is-pending'>pending</span>
                </div>
                <p className='lending-muted'>
                    {pesos(loan.principal)} {PRODUCT_LABEL[loan.product]} — awaiting setup, finish this in Your loans below.
                </p>
            </div>
        )
    }

    const next = nextInstallment(loan)

    return (
        <div className='lending-open-loan'>
            <div className='lending-open-loan-head'>
                <span>Open loan</span>
                <span className='lending-loan-status is-active'>active</span>
            </div>

            {next ? (
                <div className='lending-open-loan-next'>
                    <span className='lending-stat-label'>Next payment</span>
                    <span className='lending-open-loan-amount'>{pesos(next.total)}</span>
                    <span className='lending-muted'>Due {formatDate(next.dueAt)} · {next.installment} of {loan.schedule.length}</span>
                </div>
            ) : (
                <p className='lending-muted'>Nothing due on this loan right now.</p>
            )}

            <div className='lending-open-loan-rows'>
                <div><span>Outstanding principal</span><span>{pesos(loan.principal_outstanding)}</span></div>
                <div><span>Rate · term</span><span>{rate(loan.rate_bps)} · {loan.term_months} mo</span></div>
            </div>

            <button type='button' className='lending-btn-primary' onClick={() => navigate('/pay')}>Pay this loan</button>

            {loan.schedule.length > 0 && (
                <button type='button' className='lending-open-loan-toggle' onClick={() => setScheduleOpen(o => !o)}>
                    {scheduleOpen ? 'Hide details' : 'View details'} {scheduleOpen ? <ChevronUp /> : <ChevronDown />}
                </button>
            )}

            {scheduleOpen && (
                <div className='lending-schedule-scroll'>
                    <table className='lending-schedule'>
                        <thead>
                            <tr><th>Due</th><th>Amount</th><th>Status</th></tr>
                        </thead>
                        <tbody>
                            {loan.schedule.map(row => (
                                <tr key={row.installment} className={row.status === 'paid' ? 'is-paid' : undefined}>
                                    <td>{formatDate(row.due_at)}</td>
                                    <td>{pesos(row.principal_due + row.interest_due)}</td>
                                    <td>{next?.installment === row.installment ? 'due next' : row.status}</td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </div>
            )}
        </div>
    )
}

export default OpenLoanCard
