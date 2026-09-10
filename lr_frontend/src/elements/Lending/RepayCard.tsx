import { lazy, Suspense } from 'react'
import { useNavigate } from 'react-router-dom'
import { ClipboardList, CircleCheckBig, Wallet, Check } from 'lucide-react'
import { formatDate, pesos, rate } from '../../functions/Lending/money'
import { PRODUCT_LABEL, type Loan, type PoolResponse } from '../../functions/Lending/types'
import type { RepayRef } from '../../functions/Lending/useLoans'
import { RepayCardBody } from './PaySkeleton'

// Lazy so the page shell (and the summary/history cards next to it) paint
// before the PayPal SDK bootstrap loads.
const PayPalButton = lazy(() => import('./PayPalButton'))
// No SDK behind this one — it asks the engine for a Checkout Session and
// navigates — so it doesn't need the lazy treatment PayPal gets.
import StripeButton from './StripeButton'

const STATUS_CLS: Record<Loan['status'], string> = {
    pending: 'is-pending',
    active: 'is-active',
    closed: 'is-closed',
    defaulted: 'is-defaulted',
    reconciling: 'is-pending',
    reconciled: 'is-closed',
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
    repay: (loanId: string, ref: RepayRef) => Promise<boolean>
    repayingId: string | null
    onPaid: () => void
}) {
    const navigate = useNavigate()
    // `reconciling` is a defaulted loan an administrator reopened so the
    // borrower can settle it (033). It pays through this same card and the same
    // rail — the only differences are what the money does in the books, which
    // is the engine's business, and the wording below, which is the borrower's.
    const activeLoan = loans.find(l => l.status === 'active' || l.status === 'reconciling') ?? null
    const settling = activeLoan?.status === 'reconciling'
    const next = activeLoan ? nextInstallment(activeLoan) : null
    /** What a settling borrower still owes. The engine's number, never summed
     *  from the schedule here: settling doesn't mean paying every remaining
     *  month over again — the recovery waterfall already took what it could
     *  from the borrower's own deposit, and only what other people are still
     *  short is left to repay (033). The schedule can't answer that. */
    const arrears = activeLoan?.arrears ?? 0
    /** What the pay button charges, and — at 0 — whether it appears at all.
     *  A settling loan pays its arrears; an ordinary one pays its next
     *  installment. Driving the settlement off the schedule is what let a
     *  fully-settled loan keep taking payments: the schedule still shows the
     *  defaulted month as due, and always will, because settling deliberately
     *  doesn't rewrite it. */
    const payNow = settling ? arrears : (next?.total ?? 0)

    return (
        <section className='lending-card lending-card-repay'>
            <div className='lending-card-head'>
                <span className='lending-card-icon is-accent'><ClipboardList /></span>
                <h2>Repay your loan</h2>
            </div>

            {loading ? (
                <RepayCardBody />
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
                        <span className={`lending-loan-status ${STATUS_CLS[activeLoan.status]}`}>
                            {settling ? 'settling' : activeLoan.status}
                        </span>
                    </div>

                    {/* A settling borrower needs to know three things the
                        ordinary repay flow never has to say: that this is the
                        defaulted loan, what the whole debt is (not just this
                        month), and that clearing it is what gets them their
                        standing back. */}
                    {settling && (
                        <p className='lending-muted'>
                            This loan defaulted and has been reopened so you can settle it.
                            {arrears > 0
                                ? <> You owe <b>{pesos(arrears)}</b> — not the whole loan: what your own
                                    deposit already covered when it defaulted isn’t charged again. You can pay
                                    it in parts.</>
                                : <> Nothing is outstanding; an administrator will confirm the settlement.</>}
                            {' '}Once it’s confirmed, the credit penalty is returned and you can apply again.
                        </p>
                    )}

                    <div className='lending-funds-grid'>
                        {/* Not shown while settling: recovery wrote
                            `principal_outstanding` down to zero when the loan
                            defaulted, so this tile reads ₱0.00 next to a real
                            amount still to pay — two numbers that contradict
                            each other. "Left to settle" is the honest one. */}
                        {!settling && (
                            <div className='lending-funds-tile'>
                                <span className='lending-stat-label'>Outstanding</span>
                                <span className='lending-stat-value'>{pesos(activeLoan.principal_outstanding)}</span>
                            </div>
                        )}
                        {/* A settlement is not an installment. Its schedule was
                            left exactly as the default stamped it, so the
                            "next installment" tiles below would keep pointing
                            at a defaulted month forever — which is what made
                            this card offer to charge for it over and over. A
                            settling loan gets one tile: what is left to pay. */}
                        {settling ? (
                            arrears > 0 && (
                                <div className='lending-funds-tile is-highlight'>
                                    <span className='lending-stat-label'>Left to settle</span>
                                    <span className='lending-stat-value'>{pesos(arrears)}</span>
                                </div>
                            )
                        ) : (
                            next && (
                                <>
                                    <div className='lending-funds-tile is-highlight'>
                                        <span className='lending-stat-label'>Installment {next.installment} due</span>
                                        <span className='lending-stat-value'>{pesos(next.total)}</span>
                                    </div>
                                    <div className='lending-funds-tile'>
                                        <span className='lending-stat-label'>Principal</span>
                                        <span className='lending-stat-value'>{pesos(next.principal)}</span>
                                    </div>
                                    <div className='lending-funds-tile'>
                                        <span className='lending-stat-label'>Interest</span>
                                        <span className='lending-stat-value'>{pesos(next.interest)}</span>
                                    </div>
                                </>
                            )
                        )}
                    </div>

                    {activeLoan.disbursed_at && (
                        <p className='lending-muted'>Disbursed {formatDate(activeLoan.disbursed_at)}</p>
                    )}

                    {payNow > 0 ? (
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
                                    amountCentavos={payNow}
                                    purpose='repay'
                                    loanId={activeLoan.id}
                                    onApproved={async orderId => {
                                        if (await repay(activeLoan.id, { order_id: orderId })) onPaid()
                                    }}
                                />
                            </Suspense>
                            {/* The card rail. Same money, same books — a
                                second way to pay for borrowers without PayPal,
                                and the one the deposit form has offered since
                                the Stripe rail landed. This card was the last
                                place still PayPal-only. */}
                            <StripeButton
                                amountCentavos={payNow}
                                purpose='repay'
                                loanId={activeLoan.id}
                                label={`Pay ${pesos(payNow)} by card`}
                            />
                            {repayingId === activeLoan.id && <p className='lending-muted'>Applying your payment…</p>}
                        </>
                    ) : settling ? (
                        <p className='lending-muted'>
                            Nothing left to pay — an administrator will confirm the settlement and restore
                            your standing. You don’t need to do anything else.
                        </p>
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
