import { Link, useLocation } from 'react-router-dom'
import { ChevronLeft, ChevronRight } from 'lucide-react'
import { formatDate, pesos, pesosCompact, rate, xlm } from '../../functions/Lending/money'
import { PRODUCT_LABEL } from '../../functions/Lending/types'
import {
    PUBLIC_STATUS_LABEL,
    type ProductFilter,
    type PublicLoan,
    type PublicLoansPage,
    type StatusFilter,
} from '../../functions/Lending/publicLoans'
import { STATUS_CLS, loanRef } from './recordLabels'
import { PagerSkeleton, SkeletonBone } from '../Lending/Skeleton'
import { RecordRowsSkeleton, RecordTilesSkeleton } from './RecordsSkeleton'

const STATUS_FILTERS: { key: StatusFilter; label: string }[] = [
    { key: '', label: 'All' },
    { key: 'pending', label: 'Pending' },
    { key: 'active', label: 'Active' },
    { key: 'closed', label: 'Repaid' },
    { key: 'defaulted', label: 'Defaulted' },
    { key: 'declined', label: 'Declined' },
    { key: 'cancelled', label: 'Cancelled' },
]

const PRODUCT_FILTERS: { key: ProductFilter; label: string }[] = [
    { key: '', label: 'Every product' },
    { key: 'deposit_backed', label: 'Deposit-backed' },
    { key: 'xlm_collateral', label: 'XLM collateral' },
    { key: 'guarantor', label: 'Guarantor' },
]

/** What stood behind a loan, as one line: only the legs it actually has. */
function backingLine(loan: PublicLoan) {
    const parts: string[] = []
    if (loan.deposit_locked > 0) parts.push(`${pesosCompact(loan.deposit_locked)} own deposit`)
    if (loan.xlm_required_stroops !== null) {
        parts.push(`${xlm(loan.xlm_locked_stroops || loan.xlm_required_stroops)}${loan.xlm_locked_stroops ? '' : ' requested'}`)
    }
    if (loan.guarantors > 0) {
        parts.push(`${pesosCompact(loan.guarantor_locked)} from ${loan.guarantors} ${loan.guarantors === 1 ? 'guarantor' : 'guarantors'}`)
    }
    return parts.length > 0 ? parts.join(' · ') : 'Nothing locked'
}

type Props = {
    result: { data: PublicLoansPage | null; loading: boolean; error: boolean }
    status: StatusFilter
    product: ProductFilter
    onFilter: (next: { status?: StatusFilter; product?: ProductFilter; page?: number }) => void
}

/**
 * The book: its totals, the two filters, and one row per application. A row
 * is a link to that loan's whole record, carrying the list's query along so
 * "back" lands on the same page of the same filter.
 */
function RecordsList({ result, status, product, onFilter }: Props) {
    const { data, loading, error } = result
    const { search } = useLocation()

    return (
        <>
            {/* The totals are the whole book's whatever the filter, so a
                page turn keeps the last ones up; only the first load has none. */}
            {!data && loading && <RecordTilesSkeleton />}
            {data && (
                <div className='lending-pool-tiles'>
                    <div className='lending-funds-tile'>
                        <span className='lending-stat-label'>Applications</span>
                        <span className='lending-stat-value'>{data.summary.loans.toLocaleString()}</span>
                    </div>
                    <div className='lending-funds-tile'>
                        <span className='lending-stat-label'>Disbursed</span>
                        <span className='lending-stat-value'>{pesosCompact(data.summary.disbursed)}</span>
                    </div>
                    <div className='lending-funds-tile'>
                        <span className='lending-stat-label'>Repaid</span>
                        <span className='lending-stat-value'>{pesosCompact(data.summary.repaid)}</span>
                    </div>
                    <div className='lending-funds-tile is-good'>
                        <span className='lending-stat-label'>Interest collected</span>
                        <span className='lending-stat-value is-good'>{pesosCompact(data.summary.interest)}</span>
                    </div>
                </div>
            )}

            <section className='lending-card'>
                <div className='records-filters'>
                    <div className='lending-tab-group' role='group' aria-label='Filter by status'>
                        {STATUS_FILTERS.map(f => {
                            const count = f.key === ''
                                ? data?.summary.loans
                                : data?.summary.by_status
                                    .filter(s => (f.key === 'defaulted'
                                        ? ['defaulted', 'reconciling', 'reconciled'].includes(s.status)
                                        : s.status === f.key))
                                    .reduce((sum, s) => sum + s.count, 0)
                            return (
                                <button
                                    key={f.key || 'all'}
                                    type='button'
                                    className={`lending-tab${status === f.key ? ' is-active' : ''}`}
                                    aria-pressed={status === f.key}
                                    onClick={() => onFilter({ status: f.key, page: 1 })}
                                >
                                    {f.label}
                                    {count !== undefined && <span className='records-count'>{count}</span>}
                                </button>
                            )
                        })}
                    </div>
                    <div className='lending-tab-group' role='group' aria-label='Filter by product'>
                        {PRODUCT_FILTERS.map(f => (
                            <button
                                key={f.key || 'all'}
                                type='button'
                                className={`lending-tab${product === f.key ? ' is-active' : ''}`}
                                aria-pressed={product === f.key}
                                onClick={() => onFilter({ product: f.key, page: 1 })}
                            >
                                {f.label}
                            </button>
                        ))}
                    </div>
                </div>

                {loading ? (
                    // First load, a page turn or a filter change alike: the
                    // rows are for a query that hasn't answered yet, so they
                    // are bones rather than the previous page's loans.
                    <>
                        <p className='lending-muted'><SkeletonBone width={130} height={13} /></p>
                        <RecordRowsSkeleton />
                        {data && data.total_pages > 1 ? <PagerSkeleton /> : null}
                    </>
                ) : error ? (
                    <p className='lending-muted'>Couldn’t load the loan book. Please try again later.</p>
                ) : !data || data.total === 0 ? (
                    <p className='lending-muted'>No loans match this filter.</p>
                ) : (
                    <>
                        <p className='lending-muted'>
                            {data.total.toLocaleString()} {data.total === 1 ? 'loan' : 'loans'} · newest first
                        </p>
                        <ul className='lending-loans'>
                            {data.items.map(loan => (
                                <li key={loan.id} className='lending-loan'>
                                    <Link
                                        to={`/records/${loan.id}`}
                                        state={{ from: search }}
                                        className='lending-loan-summary records-row'
                                    >
                                        <div className='lending-loan-title'>
                                            <b>
                                                {pesos(loan.principal)}
                                                <span className='records-ref'>#{loanRef(loan.id)}</span>
                                            </b>
                                            <span>
                                                {PRODUCT_LABEL[loan.product] ?? loan.product} · {rate(loan.rate_bps)} ·{' '}
                                                {loan.term_months} months · applied {formatDate(loan.applied_at)}
                                            </span>
                                            <span>
                                                {backingLine(loan)}
                                                {loan.payments > 0 && (
                                                    <> · {pesosCompact(loan.repaid)} repaid in {loan.payments}{' '}
                                                        {loan.payments === 1 ? 'payment' : 'payments'}</>
                                                )}
                                            </span>
                                        </div>
                                        <span className={`lending-loan-status ${STATUS_CLS[loan.status]}`}>
                                            {PUBLIC_STATUS_LABEL[loan.status] ?? loan.status}
                                        </span>
                                        <ChevronRight aria-hidden='true' />
                                    </Link>
                                </li>
                            ))}
                        </ul>

                        {data.total_pages > 1 && (
                            <div className='lending-pager'>
                                <button
                                    type='button'
                                    className='lending-pager-btn'
                                    aria-label='Previous page'
                                    disabled={data.page <= 1}
                                    onClick={() => onFilter({ page: data.page - 1 })}
                                >
                                    <ChevronLeft />
                                </button>
                                <span className='lending-muted'>Page {data.page} of {data.total_pages}</span>
                                <button
                                    type='button'
                                    className='lending-pager-btn'
                                    aria-label='Next page'
                                    disabled={data.page >= data.total_pages}
                                    onClick={() => onFilter({ page: data.page + 1 })}
                                >
                                    <ChevronRight />
                                </button>
                            </div>
                        )}
                    </>
                )}
            </section>
        </>
    )
}

export default RecordsList
