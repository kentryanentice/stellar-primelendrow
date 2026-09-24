import { pesosCompact, sharePct } from '../../functions/Lending/money'
import type { PoolResponse } from '../../functions/Lending/types'

/**
 * The pool's health in one strip — the same four figures PoolOverviewCard
 * tiles out on the Lend page, compressed so they sit under the member's own
 * numbers without competing with them.
 */
function PoolStrip({ data }: { data: PoolResponse }) {
    const { pool } = data

    return (
        <section className='lending-card dash-pool' aria-label='Lending pool'>
            <div className='dash-pool-stat'>
                <span className='lending-stat-label'>Pool size</span>
                <span className='dash-pool-value'>{pesosCompact(pool.total_deposits)}</span>
            </div>
            <span className='dash-pool-divider' aria-hidden='true' />
            <div className='dash-pool-stat'>
                <span className='lending-stat-label'>Out on loans</span>
                <span className='dash-pool-value is-warn'>{pesosCompact(pool.out_on_loans)}</span>
            </div>
            <span className='dash-pool-divider' aria-hidden='true' />
            <div className='dash-pool-stat'>
                <span className='lending-stat-label'>Cash available</span>
                <span className='dash-pool-value'>{pesosCompact(pool.cash_available)}</span>
                {pool.pool_funds > 0 && (
                    <span className='lending-muted'>+ {pesosCompact(pool.pool_funds)} platform & risk funds held</span>
                )}
            </div>
            <span className='dash-pool-divider' aria-hidden='true' />
            <div className='dash-pool-stat dash-pool-working'>
                <span className='lending-stat-label'>Pool working</span>
                <div className='lending-pool-working'>
                    <span className='dash-pool-value is-good'>{sharePct(pool.utilization_bps)}</span>
                    <div className='lending-pool-working-track'>
                        <div className='lending-pool-working-fill' style={{ width: `${Math.min(100, pool.utilization_bps / 100)}%` }} />
                    </div>
                </div>
            </div>
        </section>
    )
}

export default PoolStrip
