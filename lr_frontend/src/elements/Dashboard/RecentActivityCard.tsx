import { useState } from 'react'
import { Link } from 'react-router-dom'
import { ArrowDown, ArrowUp, Check, History, Lock, LockOpen, ShieldAlert, Undo2, type LucideIcon } from 'lucide-react'
import { formatDate } from '../../functions/Lending/money'
import type { ActivityGlyph, ActivityRow, ActivitySide } from '../../functions/Dashboard/overview'
import { ActivityRowsSkeleton } from './DashboardSkeleton'

const GLYPH_ICON: Record<ActivityGlyph, LucideIcon> = {
    deposit: ArrowUp,
    withdraw: ArrowDown,
    refund: Undo2,
    seized: ShieldAlert,
    lock: Lock,
    unlock: LockOpen,
    paid: Check,
    disbursed: ArrowDown,
}

type Filter = 'all' | ActivitySide

const FILTERS: { key: Filter; label: string }[] = [
    { key: 'all', label: 'All' },
    { key: 'lending', label: 'Lending' },
    { key: 'borrowing', label: 'Borrowing' },
]

const EMPTY: Record<Filter, string> = {
    all: 'Nothing yet — deposits, withdrawals, loans and payments all show up here.',
    lending: 'No pool activity yet — deposits and withdrawals show up here.',
    borrowing: 'No borrowing activity yet — loans and repayments show up here.',
}

/** Where the complete history for each filter lives. */
const FULL_HISTORY: Record<Filter, string> = {
    all: '/lending',
    lending: '/lending',
    borrowing: '/pay',
}

const LIMIT = 6

/**
 * The newest few movements across lending and borrowing. A glance, not the
 * record — the full paginated histories stay on Lend (transactions) and Pay
 * (repayments), which the footer link points at.
 */
function RecentActivityCard({ rows, loading, error }: { rows: ActivityRow[]; loading: boolean; error: boolean }) {
    const [filter, setFilter] = useState<Filter>('all')
    const visible = rows.filter(row => filter === 'all' || row.side === filter).slice(0, LIMIT)

    return (
        <section className='lending-card'>
            <div className='lending-card-head dash-activity-head'>
                <span className='lending-card-icon is-accent'><History /></span>
                <h2>Recent activity</h2>
                <div className='lending-tab-group dash-filters' role='group' aria-label='Filter activity'>
                    {FILTERS.map(f => (
                        <button
                            key={f.key}
                            type='button'
                            aria-pressed={filter === f.key}
                            className={`lending-tab${filter === f.key ? ' is-active' : ''}`}
                            onClick={() => setFilter(f.key)}
                        >
                            {f.label}
                        </button>
                    ))}
                </div>
            </div>

            {loading ? (
                <ActivityRowsSkeleton rows={LIMIT} />
            ) : error ? (
                <p className='lending-muted'>Couldn’t load your activity. Please try again later.</p>
            ) : visible.length === 0 ? (
                <p className='lending-muted'>{EMPTY[filter]}</p>
            ) : (
                <div>
                    <div className='dash-activity-cols' aria-hidden='true'>
                        <span>Activity</span>
                        <span>Status</span>
                        <span>Date</span>
                    </div>
                    <ul className='dash-activity-list'>
                        {visible.map(row => {
                            const Icon = GLYPH_ICON[row.glyph]
                            return (
                                <li key={row.key} className='dash-activity-row'>
                                    <span className='dash-activity-icon'><Icon aria-hidden='true' /></span>
                                    <span className='dash-activity-what'>
                                        <b>{row.amount}</b>
                                        <span title={row.detail}>{row.detail}</span>
                                    </span>
                                    <span className='dash-activity-status'>
                                        <span className={`lending-tx-status ${row.tone}`}>{row.status}</span>
                                    </span>
                                    <span className='dash-activity-date'>{formatDate(row.at)}</span>
                                </li>
                            )
                        })}
                    </ul>
                </div>
            )}

            <div className='dash-activity-foot'>
                <span className='lending-muted'>Locked deposits free up automatically as the loans they fund are repaid.</span>
                <Link to={FULL_HISTORY[filter]} className='dash-link'>See all activity</Link>
            </div>
        </section>
    )
}

export default RecentActivityCard
