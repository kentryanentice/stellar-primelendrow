import { History, ChevronLeft, ChevronRight } from 'lucide-react'
import type useDeposits from '../../functions/Lending/useDeposits'
import { formatDate, pesos } from '../../functions/Lending/money'
import type { LotBadge } from '../../functions/Lending/types'
import { LedgerRowsSkeleton, PagerSkeleton } from './Skeleton'

const BADGE_META: Record<LotBadge, { label: string; cls: string }> = {
    available: { label: 'Withdrawable', cls: 'is-available' },
    lent: { label: 'Funding a loan', cls: 'is-lent' },
    collateral: { label: 'Backing your loan', cls: 'is-collateral' },
    pledged: { label: 'Pledged for a friend', cls: 'is-pledged' },
}

/**
 * The caller's deposit lots, one page at a time (POST /pool/deposits) — the
 * "withdrawable unless it's funding a loan" rule made visible, lot by lot, as
 * a ledger table. Paginated server-side (fixed page size) rather than
 * fetched in one shot, since a long-standing member's lot history only grows.
 */
function YourDepositsCard({ deposits }: { deposits: ReturnType<typeof useDeposits> }) {
    const { lots, page, total, totalPages, loading, error, goToPage } = deposits

    return (
        <section className='lending-card lending-card-deposits'>
            <div className='lending-card-head'>
                <span className='lending-card-icon is-accent'><History /></span>
                <h2>Your deposits</h2>
                {total > 0 && <span className='lending-muted lending-ledger-count'>{total} {total === 1 ? 'deposit' : 'deposits'}</span>}
            </div>

            {loading ? (
                <>
                    <div className='lending-rates-scroll'>
                        <table className='lending-ledger-table' aria-hidden='true'>
                            <thead>
                                <tr>
                                    <th>Amount</th>
                                    <th>Status</th>
                                    <th>Deposited</th>
                                </tr>
                            </thead>
                            <tbody>
                                <LedgerRowsSkeleton />
                            </tbody>
                        </table>
                    </div>
                    {totalPages > 1 && <PagerSkeleton />}
                </>
            ) : error ? (
                <p className='lending-muted'>Couldn’t load your deposits. Please try again later.</p>
            ) : total === 0 ? (
                <p className='lending-muted'>No deposits yet — once you deposit into the pool, each lot shows up here.</p>
            ) : (
                <>
                    <div className='lending-rates-scroll'>
                        <table className='lending-ledger-table'>
                            <thead>
                                <tr>
                                    <th>Amount</th>
                                    <th>Status</th>
                                    <th>Deposited</th>
                                </tr>
                            </thead>
                            <tbody>
                                {lots.map(lot => {
                                    const meta = BADGE_META[lot.badge]
                                    return (
                                        <tr key={lot.id}>
                                            <td className='lending-ledger-amount'>{pesos(lot.amount)}</td>
                                            <td><span className={`lending-lot-badge ${meta.cls}`}>{meta.label}</span></td>
                                            <td>{formatDate(lot.created_at)}</td>
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

                    <p className='lending-muted'>
                        Locked deposits free up automatically as the loans they fund are repaid.
                    </p>
                </>
            )}
        </section>
    )
}

export default YourDepositsCard
