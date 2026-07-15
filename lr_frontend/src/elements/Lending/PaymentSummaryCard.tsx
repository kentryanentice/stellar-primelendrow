import { History } from 'lucide-react'
import { pesos } from '../../functions/Lending/money'
import type { PaymentTotals } from '../../functions/Lending/usePayments'

/**
 * All-time repayment totals (POST /loans/payments' `totals` — summed across
 * every payment, not just the page the history card below shows). The hero
 * number + breakdown reuse EligibilityCard's visual language.
 */
function PaymentSummaryCard({ totals, count }: { totals: PaymentTotals; count: number }) {
    return (
        <section className='lending-card lending-card-payment-summary'>
            <div className='lending-card-head'>
                <span className='lending-card-icon is-accent'><History /></span>
                <h2>Repaid to date</h2>
            </div>

            <div className='lending-eligibility-score'>
                <span className='lending-eligibility-score-value'>{pesos(totals.amount_received)}</span>
                <span className='lending-eligibility-score-max'>· {count} {count === 1 ? 'payment' : 'payments'}</span>
            </div>

            <div className='lending-eligibility-rows'>
                <div className='lending-quote-row'>
                    <span>Principal repaid</span>
                    <b>{pesos(totals.principal_paid)}</b>
                </div>
                <div className='lending-quote-row'>
                    <span>Interest paid</span>
                    <b>{pesos(totals.interest_paid)}</b>
                </div>
            </div>
        </section>
    )
}

export default PaymentSummaryCard
