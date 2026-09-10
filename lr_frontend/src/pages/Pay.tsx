import { lazy, Suspense, useEffect } from 'react'
import useLendingPool from '../functions/Lending/useLendingPool'
import useLoans from '../functions/Lending/useLoans'
import usePayments from '../functions/Lending/usePayments'
import { stripeCheckoutResult } from '../functions/Lending/useStripeCheckout'
import { useToast } from '../providers/useToast'
import PaymentSummaryCard from '../elements/Lending/PaymentSummaryCard'
import PaySkeleton, { RepayCardSkeleton, PaymentHistoryCardSkeleton } from '../elements/Lending/PaySkeleton'

// RepayCard pulls the wallet-adjacent PayPal SDK bootstrap — lazy so the page
// shell (and the summary/history cards) paint first.
const RepayCard = lazy(() => import('../elements/Lending/RepayCard'))
const PaymentHistoryCard = lazy(() => import('../elements/Lending/PaymentHistoryCard'))

/**
 * The Pay page: settle the next installment on the caller's one open loan —
 * with PayPal or by card — plus the full repayment history. The engine
 * verifies and allocates server-side; this page only ever hands it a
 * reference to a payment the provider confirmed.
 */
function Pay() {
    const { data, loading: poolLoading, error: poolError, refresh } = useLendingPool()
    const { loans, loading: loansLoading, error: loansError, repay, repayingId } = useLoans()
    const payments = usePayments()
    const toast = useToast()

    // A repayment can change the pool's badge totals (excess -> a fresh
    // deposit lot), the loan itself, and the payment history — refresh
    // everything so no card is left showing a stale number.
    const handlePaid = () => {
        refresh()
        payments.refresh()
    }

    // Coming back from Stripe Checkout. A card repayment is confirmed on page
    // *load*, because the borrower has been away paying on stripe.com — unlike
    // PayPal, where approval happens in an iframe and the page never unloads.
    //
    // The loan comes from the URL rather than from `loans`, because the engine
    // put it there when it built the return path (`/pay?loan=<id>`) and this
    // effect must not wait for the loan list to arrive. `stripeCheckoutResult`
    // consumes the query parameter on its first read, so React's development
    // double-invoke can't submit the same session twice — and the ledger's
    // unique rail_ref would refuse it even if it did.
    useEffect(() => {
        const result = stripeCheckoutResult()
        if (!result) return
        if ('cancelled' in result) {
            toast.error('Payment cancelled — nothing was charged')
            return
        }
        const loanId = new URLSearchParams(window.location.search).get('loan')
        if (!loanId) {
            toast.error('That payment came back without a loan on it — check your payment history')
            return
        }
        void repay(loanId, { session_id: result.sessionId }).then(ok => {
            if (ok) handlePaid()
        })
        // `repay` and `handlePaid` are recreated every render; the effect is
        // safe to re-run because the query parameter is gone after the first
        // read, so it returns immediately on every subsequent pass.
    })

    return (
        <main className='lending-page'>
            <header className='lending-head'>
                <p className='lending-eyebrow'>Payments</p>
                <h1>Repay your loan</h1>
                <p>Interest is charged on the remaining balance, so paying early costs less over time.</p>
            </header>

            {poolLoading ? (
                <PaySkeleton />
            ) : poolError || !data ? (
                <section className='lending-card'>
                    <p className='lending-muted'>Couldn’t load the pool. Please try again later.</p>
                    <button type='button' className='lending-btn' onClick={refresh}>Retry</button>
                </section>
            ) : (
                <div className='lending-columns'>
                    <div className='lending-column'>
                        <Suspense fallback={<RepayCardSkeleton />}>
                            <RepayCard
                                data={data}
                                loans={loans}
                                loading={loansLoading}
                                error={loansError}
                                repay={repay}
                                repayingId={repayingId}
                                onPaid={handlePaid}
                            />
                        </Suspense>
                    </div>
                    <div className='lending-column'>
                        <PaymentSummaryCard totals={payments.totals} count={payments.total} />
                        <Suspense fallback={<PaymentHistoryCardSkeleton />}>
                            <PaymentHistoryCard payments={payments} />
                        </Suspense>
                    </div>
                </div>
            )}
        </main>
    )
}

export default Pay
