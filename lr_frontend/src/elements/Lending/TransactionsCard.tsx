import { ArrowLeftRight, ChevronLeft, ChevronRight, ExternalLink } from 'lucide-react'
import type useTransactions from '../../functions/Lending/useTransactions'
import { formatDate, pesos, xlm } from '../../functions/Lending/money'
import { shortId, txLink } from '../../functions/Lending/explorer'
import { TRANSACTION_KIND_LABEL, TRANSACTION_STATUS_META, type Transaction } from '../../functions/Lending/types'
import { TransactionRowsSkeleton, PagerSkeleton } from './Skeleton'

/** On-chain references are checkable on a block explorer; a PayPal capture or
 *  transfer id is only checkable inside PayPal, so it's shown as plain text
 *  rather than dressed up as a link that goes nowhere useful. */
function Reference({ tx }: { tx: Transaction }) {
    if (!tx.reference) return null
    if (tx.asset === 'xlm') {
        return (
            <a
                className='lending-tx-ref'
                href={txLink(tx.reference)}
                target='_blank'
                rel='noopener noreferrer'
            >
                <span>{shortId(tx.reference)}</span>
                <ExternalLink aria-hidden='true' />
            </a>
        )
    }
    return <span className='lending-tx-ref'>{shortId(tx.reference)}</span>
}

/**
 * Every movement of the member's money, newest first (POST /pool/transactions)
 * — pesos into the pool and back out, XLM into the vault contract and back
 * out. The one place on the site that answers "what has actually happened to
 * my money", as opposed to "what do I hold right now" (YourDepositsCard) or
 * "what happened to this one loan's collateral" (CollateralRecordCard).
 *
 * It has to exist separately from the deposits list because a withdrawal
 * DELETES the lots it consumes: after taking money out, there is no row left
 * anywhere on this page saying it ever came in. The ledger keeps the story;
 * this card is the window onto it.
 *
 * Every row carries the provider's own reference — a PayPal id or a Stellar
 * transaction hash — so none of it has to be taken on trust.
 */
function TransactionsCard({ transactions }: { transactions: ReturnType<typeof useTransactions> }) {
    const { items, page, total, totalPages, loading, error, goToPage } = transactions

    const head = (
        <thead>
            <tr>
                <th>Movement</th>
                <th>Amount</th>
                <th>Status</th>
                <th>Date</th>
            </tr>
        </thead>
    )

    return (
        <section className='lending-card lending-card-transactions'>
            <div className='lending-card-head'>
                <span className='lending-card-icon is-accent'><ArrowLeftRight /></span>
                <h2>Transaction record</h2>
                {total > 0 && (
                    <span className='lending-muted lending-ledger-count'>
                        {total} {total === 1 ? 'movement' : 'movements'}
                    </span>
                )}
            </div>

            {loading ? (
                <>
                    <div className='lending-rates-scroll'>
                        <table className='lending-ledger-table' aria-hidden='true'>
                            {head}
                            <tbody><TransactionRowsSkeleton /></tbody>
                        </table>
                    </div>
                    {totalPages > 1 && <PagerSkeleton />}
                </>
            ) : error ? (
                <p className='lending-muted'>Couldn’t load your transactions. Please try again later.</p>
            ) : total === 0 ? (
                <p className='lending-muted'>
                    Nothing yet — every deposit, withdrawal and collateral movement shows up here with its
                    PayPal or Stellar reference.
                </p>
            ) : (
                <>
                    <div className='lending-rates-scroll'>
                        <table className='lending-ledger-table'>
                            {head}
                            <tbody>
                                {items.map(tx => {
                                    const status = TRANSACTION_STATUS_META[tx.status]
                                    return (
                                        <tr key={tx.id}>
                                            <td>
                                                <span className='lending-tx-kind'>{TRANSACTION_KIND_LABEL[tx.kind]}</span>
                                                <Reference tx={tx} />
                                            </td>
                                            <td className='lending-ledger-amount'>
                                                {tx.asset === 'php' ? pesos(tx.amount) : xlm(tx.amount)}
                                            </td>
                                            <td>
                                                <span className={`lending-tx-status ${status.cls}`}>{status.label}</span>
                                            </td>
                                            <td>{formatDate(tx.at)}</td>
                                        </tr>
                                    )
                                })}
                            </tbody>
                        </table>
                    </div>

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

export default TransactionsCard
