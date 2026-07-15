import { lazy, Suspense } from 'react'
import { useNavigate } from 'react-router-dom'
import { ClipboardList, CircleCheckBig, Wallet, Check } from 'lucide-react'
import { formatDate, pesos, rate } from '../../functions/Lending/money'
import { PRODUCT_LABEL, type Loan, type PoolResponse } from '../../functions/Lending/types'

// Lazy so the page shell (and the summary/history cards next to it) paint
// before the PayPal SDK bootstrap loads.
const PayPalButton = lazy(() => import('./PayPalButton'))

const STATUS_CLS: Record<Loan['status'], string> = {
    pending: 'is-pending',
    active: 'is-active',
    closed: 'is-closed',
    defaulted: 'is-defaulted',
    declined: 'is-declined',
    cancelled: 'is-declined',
}

type NextInstallment = { installment: number; principal: number; interest: number; total: number }

/** The earliest not-fully-settled installment, with what's still owed on it
 *  split into that month's principal and interest. The engine pays interest
 *  first across every month (repay.rs), so on a partly-paid row the interest
 *  side may already read ₱0 — the split is always what's *actually* still due,
 *  which is also what the PayPal sheet is pre-filled with. Convenience, not
 *  authority: the engine re-allocates whatever actually arrives. */
const nextInstallment = (loan: Loan): NextInstallment | null => {
    for (const row of loan.schedule) {
        const interest = row.interest_due - row.interest_paid
        const principal = row.principal_due - row.principal_paid
        if (interest + principal > 0) {
            return { installment: row.installment, principal, interest, total: interest + principal }
        }
    }
    return null
}

/**
 * Settle the next installment on the caller's one open loan (D8: at most one
 * pending/active loan at a time). PayPal capture is server-side; this only
 * ever hands the engine an order id. No active loan is the common case once
 * everything's repaid — that's an empty state pointing at Borrow, not an error.
 */
function RepayCard({ data, loans, loading, error, repay, repayingId, onPaid }: {
    data: PoolResponse
    loans: Loan[]
    loading: boolean
    error: boolean
    repay: (loanId: string, orderId: string) => Promise<boolean>
    repayingId: string | null
    onPaid: () => void
}) {
    const navigate = useNavigate()
    const activeLoan = loans.find(l => l.status === 'active') ?? null
    const next = activeLoan ? nextInstallment(activeLoan) : null

    return (
        <section className='lending-card lending-card-repay'>
            <div className='lending-card-head'>
                <span className='lending-card-icon is-accent'><ClipboardList /></span>
                <h2>Repay your loan</h2>
            </div>

            {loading ? (
                <p className='lending-muted'>Loading your loan…</p>
            ) : error ? (
                <p className='lending-muted'>Couldn’t load your loan. Please try again later.</p>
            ) : !activeLoan ? (
                <div className='lending-empty'>
                    <div className='lending-empty-icon'><CircleCheckBig aria-hidden='true' /></div>
                    <p className='lending-empty-title'>You’re all paid up</p>
                    <p className='lending-muted'>No active loans to repay right now. Need funds? Start a new application.</p>
                    <button type='button' className='lending-btn-primary' onClick={() => navigate('/borrow')}>Apply for a loan</button>
                </div>
            ) : (
                <>
                    <div className='lending-pay-loan'>
                        <div className='lending-loan-title'>
                            <b>{pesos(activeLoan.principal)} loan</b>
                            <span>{PRODUCT_LABEL[activeLoan.product]} · {rate(activeLoan.rate_bps)} · {activeLoan.term_months} mo</span>
                        </div>
                        <span className={`lending-loan-status ${STATUS_CLS[activeLoan.status]}`}>{activeLoan.status}</span>
                    </div>

                    <div className='lending-funds-grid'>
                        <div className='lending-funds-tile'>
                            <span className='lending-stat-label'>Outstanding</span>
                            <span className='lending-stat-value'>{pesos(activeLoan.principal_outstanding)}</span>
                        </div>
                        {next && (
                            <div className='lending-funds-tile is-highlight'>
                                <span className='lending-stat-label'>Installment {next.installment} due</span>
                                <span className='lending-stat-value'>{pesos(next.total)}</span>
                            </div>
                        )}
                        {next && (
                            <>
                                <div className='lending-funds-tile'>
                                    <span className='lending-stat-label'>Principal</span>
                                    <span className='lending-stat-value'>{pesos(next.principal)}</span>
                                </div>
                                <div className='lending-funds-tile'>
                                    <span className='lending-stat-label'>Interest</span>
                                    <span className='lending-stat-value'>{pesos(next.interest)}</span>
                                </div>
                            </>
                        )}
                    </div>

                    {activeLoan.disbursed_at && (
                        <p className='lending-muted'>Disbursed {formatDate(activeLoan.disbursed_at)}</p>
                    )}

                    {next ? (
                        <>
                            <label className='lending-label'>Payment method</label>
                            {data.params.paypal_ready && (
                                <div className='lending-payment-method'>
                                    <span className='lending-payment-method-icon'><Wallet aria-hidden='true' /></span>
                                    <span className='lending-payment-method-info'>
                                        <b>PayPal</b>
                                        <span>Connected · balance & linked cards</span>
                                    </span>
                                    <Check className='lending-payment-method-check' aria-hidden='true' />
                                </div>
                            )}
                            <Suspense fallback={<p className='lending-muted'>Loading payment…</p>}>
                                <PayPalButton
                                    amountCentavos={next.total}
                                    description='PrimeLendRow loan repayment'
                                    onApproved={async orderId => {
                                        if (await repay(activeLoan.id, orderId)) onPaid()
                                    }}
                                />
                            </Suspense>
                            {repayingId === activeLoan.id && <p className='lending-muted'>Applying your payment…</p>}
                        </>
                    ) : (
                        <p className='lending-muted'>Nothing due on this loan right now.</p>
                    )}

                    {activeLoan.schedule.length > 0 && (
                        <>
                            <span className='lending-stat-label'>Repayment schedule</span>
                            <div className='lending-schedule-scroll'>
                                <table className='lending-schedule'>
                                    <thead>
                                        <tr><th>#</th><th>Due</th><th>Principal</th><th>Interest</th><th>Status</th></tr>
                                    </thead>
                                    <tbody>
                                        {activeLoan.schedule.map(row => (
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
                        </>
                    )}
                </>
            )}
        </section>
    )
}

export default RepayCard
