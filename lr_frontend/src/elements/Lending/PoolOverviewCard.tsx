import { Landmark } from 'lucide-react'
import { pesosCompact } from '../../functions/Lending/money'
import type { PoolResponse } from '../../functions/Lending/types'

/**
 * The pool of funds, honestly (D4): how much members have deposited, how
 * much is out working as loans, and how much cash is on hand right now. The
 * live rate card lives next to this in its own card (RateTiersCard) — split
 * out so each card answers one question.
 */
function PoolOverviewCard({ data }: { data: PoolResponse }) {
    const { pool } = data

    return (
        <section className='lending-card lending-card-pool'>
            <div className='lending-card-head'>
                <span className='lending-card-icon is-accent'><Landmark /></span>
                <h2>Pool overview</h2>
            </div>

            <div className='lending-utilization'>
                <p className='lending-muted'>{pool.utilization_pct}% of the pool is working as loans right now.</p>
                <div className='lending-utilization-track' role='img' aria-label={`${pool.utilization_pct}% of the pool is lent out`}>
                    <div className='lending-utilization-fill' style={{ width: `${Math.min(100, pool.utilization_pct)}%` }} />
                </div>
            </div>

            <div className='lending-pool-stats'>
                <div>
                    <span className='lending-stat-label'>Total deposits</span>
                    <span className='lending-stat-value'>{pesosCompact(pool.total_deposits)}</span>
                </div>
                <div>
                    <span className='lending-stat-label'>Out on loans</span>
                    <span className='lending-stat-value'>{pesosCompact(pool.out_on_loans)}</span>
                </div>
                <div>
                    <span className='lending-stat-label'>Cash available</span>
                    <span className='lending-stat-value'>{pesosCompact(pool.cash_available)}</span>
                </div>
            </div>
        </section>
    )
}

export default PoolOverviewCard
