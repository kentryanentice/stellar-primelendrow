import { pesosCompact, sharePct } from '../../functions/Lending/money'
import type { PoolResponse } from '../../functions/Lending/types'

/**
 * The pool of funds, honestly: how much members have deposited, how much is
 * out working as loans, how much cash is on hand, and how hard the pool is
 * working right now, and how much interest it has collected — standalone
 * tiles rather than one boxed card, so they read at a glance above the
 * tier/ledger content. Where that interest went is InterestSplitCard's job.
 */
function PoolOverviewCard({ data }: { data: PoolResponse }) {
    const { pool } = data

    return (
        <div className='lending-pool-tiles'>
            <div className='lending-funds-tile'>
                <span className='lending-stat-label'>Pool size</span>
                <span className='lending-stat-value'>{pesosCompact(pool.total_deposits)}</span>
                {pool.proceeds_waiting > 0 && (
                    <span className='lending-muted'>+ {pesosCompact(pool.proceeds_waiting)} loan proceeds held for borrowers</span>
                )}
            </div>
            <div className='lending-funds-tile'>
                <span className='lending-stat-label'>Out on loans</span>
                <span className='lending-stat-value is-warn'>{pesosCompact(pool.out_on_loans)}</span>
            </div>
            <div className='lending-funds-tile'>
                <span className='lending-stat-label'>Cash available</span>
                <span className='lending-stat-value'>{pesosCompact(pool.cash_available)}</span>
                {pool.pool_funds > 0 && (
                    <span className='lending-muted'>+ {pesosCompact(pool.pool_funds)} platform, reserve and recovery funds held, not lent</span>
                )}
            </div>
            <div className='lending-funds-tile'>
                <span className='lending-stat-label'>Pool working</span>
                <div className='lending-pool-working'>
                    <span className='lending-stat-value is-good'>{sharePct(pool.utilization_bps)}</span>
                    <div className='lending-pool-working-track'>
                        <div className='lending-pool-working-fill' style={{ width: `${Math.min(100, pool.utilization_bps / 100)}%` }} />
                    </div>
                </div>
            </div>
            <div className='lending-funds-tile'>
                <span className='lending-stat-label'>Interest collected</span>
                <span className='lending-stat-value is-good'>{pesosCompact(pool.interest.total)}</span>
            </div>
        </div>
    )
}

export default PoolOverviewCard
