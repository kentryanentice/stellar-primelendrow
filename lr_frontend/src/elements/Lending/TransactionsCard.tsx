import { ArrowLeftRight, ChevronLeft, ChevronRight, ExternalLink } from 'lucide-react'
import type useTransactions from '../../functions/Lending/useTransactions'
import { formatDate, pesos, xlm } from '../../functions/Lending/money'
import type { Transaction, TransactionKind, TransactionStatus } from '../../functions/Lending/types'
import { TransactionRowsSkeleton, PagerSkeleton } from './Skeleton'

const NETWORK = import.meta.env.VITE_STELLAR_NETWORK === 'public' ? 'public' : 'testnet'
const txLink = (hash: string) => `https://stellar.expert/explorer/${NETWORK}/tx/${hash}`

/** "a1b2c3…9f8e" — enough to recognise a reference, short enough for a row. */
const shortRef = (ref: string) => (ref.length > 16 ? `${ref.slice(0, 6)}…${ref.slice(-4)}` : ref)

const KIND_LABEL: Record<TransactionKind, string> = {
    deposit: 'Deposit',
    withdrawal: 'Withdrawal',
    collateral_lock: 'Collateral locked',
    collateral_release: 'Collateral released',
    collateral_seize: 'Collateral seized',
}

/**
 * Short enough for a table cell, unlike the sentence-length PAYOUT_LABEL the
 * money rail uses — a member scanning a list wants the state, not the
 * explanation. `is-warn` is reserved for the states that need them to do
 * something (accept it in PayPal, ask again).
 */
const STATUS_META: Record<TransactionStatus, { label: string; cls: string }> = {
    completed: { label: 'Completed', cls: 'is-good' },
    paid: { label: 'Paid out', cls: 'is-good' },
    confirmed: { label: 'Confirmed', cls: 'is-good' },
    sent: { label: 'Sent', cls: 'is-progress' },
    pending: { label: 'Queued', cls: 'is-progress' },
    queued: { label: 'Queued', cls: 'is-progress' },
    recorded: { label: 'Recorded', cls: 'is-progress' },
    unclaimed: { label: 'Unclaimed', cls: 'is-warn' },
    returned: { label: 'Returned', cls: 'is-warn' },
    failed: { label: 'Failed', cls: 'is-warn' },
}

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
                <span>{shortRef(tx.reference)}</span>
                <ExternalLink aria-hidden='true' />
            </a>
        )
    }
    return <span className='lending-tx-ref'>{shortRef(tx.reference)}</span>
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
                                    const status = STATUS_META[tx.status]
                                    return (
                                        <tr key={tx.id}>
                                            <td>
                                                <span className='lending-tx-kind'>{KIND_LABEL[tx.kind]}</span>
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
