import { lazy, Suspense } from 'react'
import useLendingPool from '../functions/Lending/useLendingPool'
import useLoans from '../functions/Lending/useLoans'
import usePayments from '../functions/Lending/usePayments'
import PaymentSummaryCard from '../elements/Lending/PaymentSummaryCard'
import PaySkeleton, { RepayCardSkeleton, PaymentHistoryCardSkeleton } from '../elements/Lending/PaySkeleton'

// RepayCard pulls the wallet-adjacent PayPal SDK bootstrap — lazy so the page
// shell (and the summary/history cards) paint first.
const RepayCard = lazy(() => import('../elements/Lending/RepayCard'))
const PaymentHistoryCard = lazy(() => import('../elements/Lending/PaymentHistoryCard'))

/**
 * The Pay page: settle the next installment on the caller's one open loan
 * with PayPal (moved off the Borrow page's "Your loans", which is now
 * read-only), plus the full repayment history. The engine captures and
 * allocates server-side — this page only ever hands it an order id.
 */
function Pay() {
    const { data, loading: poolLoading, error: poolError, refresh } = useLendingPool()
    const { loans, loading: loansLoading, error: loansError, repay, repayingId } = useLoans()
    const payments = usePayments()

    // A repayment can change the pool's badge totals (excess -> a fresh
    // deposit lot), the loan itself, and the payment history — refresh
    // everything so no card is left showing a stale number.
    const handlePaid = () => {
        refresh()
        payments.refresh()
    }

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
