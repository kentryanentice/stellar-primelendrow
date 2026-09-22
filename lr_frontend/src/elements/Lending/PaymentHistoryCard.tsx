import { useState } from 'react'
import { Receipt as ReceiptIcon, Check, ChevronDown, ChevronLeft, ChevronRight } from 'lucide-react'
import type usePayments from '../../functions/Lending/usePayments'
import { formatDate, pesos } from '../../functions/Lending/money'
import { paymentFeeNote } from '../../functions/Lending/fees'
import { paymentReceipt } from '../../functions/Lending/receipt'
import Receipt from './Receipt'
import { PaymentRowsSkeleton, PagerSkeleton } from './Skeleton'

/**
 * The caller's payments, one page at a time (POST /loans/payments) — each
 * row is one captured PayPal payment with the engine's own interest/principal
 * split. Shares its data source with PaymentSummaryCard's all-time totals,
 * but only ever renders the page on screen.
 */
function PaymentHistoryCard({ payments }: { payments: ReturnType<typeof usePayments> }) {
    const { payments: items, page, total, totalPages, loading, error, goToPage } = payments
    /** The one payment whose receipt is open. */
    const [openId, setOpenId] = useState<number | null>(null)

    return (
        <section className='lending-card lending-card-payment-history'>
            <div className='lending-card-head'>
                <span className='lending-card-icon is-accent'><ReceiptIcon /></span>
                <h2>Payment history</h2>
            </div>

            {loading ? (
                <>
                    <ul className='lending-payment-history' aria-hidden='true'>
                        <PaymentRowsSkeleton />
                    </ul>
                    {totalPages > 1 && <PagerSkeleton />}
                </>
            ) : error ? (
                <p className='lending-muted'>Couldn’t load your payment history. Please try again later.</p>
            ) : total === 0 ? (
                <p className='lending-muted'>No payments yet — once you repay, every payment shows up here.</p>
            ) : (
                <>
                    <ul className='lending-payment-history'>
                        {items.map(pmt => {
                            const open = openId === pmt.id
                            return (
                                <li key={pmt.id} className={`lending-payment-row${open ? ' is-open' : ''}`}>
                                    <span className='lending-payment-row-icon'><Check aria-hidden='true' /></span>
                                    <span className='lending-payment-row-info'>
                                        <b>{pesos(pmt.amount_received)}</b>
                                        <span>{formatDate(pmt.paid_at)}</span>
                                    </span>
                                    <span className='lending-payment-row-split'>
                                        <span>{pesos(pmt.principal_paid)} principal</span>
                                        <span className='is-interest'>{pesos(pmt.interest_paid)} interest</span>
                                        {/* Paid on top of the installment for the
                                            payment provider (engine 043) — not
                                            part of what reached the loan. */}
                                        {paymentFeeNote(pmt) && (
                                            <span className='is-fee'>+ {paymentFeeNote(pmt)}</span>
                                        )}
                                    </span>
                                    <button
                                        type='button'
                                        className='lending-payment-toggle'
                                        aria-expanded={open}
                                        aria-label={`${open ? 'Hide' : 'Show'} receipt for the payment on ${formatDate(pmt.paid_at)}`}
                                        onClick={() => setOpenId(open ? null : pmt.id)}
                                    >
                                        <ChevronDown aria-hidden='true' />
                                    </button>
                                    {open && <Receipt lines={paymentReceipt(pmt)} />}
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

export default PaymentHistoryCard
