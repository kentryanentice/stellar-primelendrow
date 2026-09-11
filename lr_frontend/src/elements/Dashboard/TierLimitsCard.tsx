import { pesosCompact, rate } from '../../functions/Lending/money'
import type { PoolResponse } from '../../functions/Lending/types'

/**
 * The caller's own policy band, as three figures — the compact form of
 * RateTiersCard's summary row, sized for a narrow column. The full ladder
 * stays on the Lend page.
 */
function TierLimitsCard({ data }: { data: PoolResponse }) {
    const { me, params } = data
    const band = params.policy.bands.find(b => me.score >= b.min_score && me.score <= b.max_score) ?? null
    if (!band) return null

    return (
        <section className='lending-card'>
            <span className='lending-stat-label'>Your tier limits</span>
            <div className='dash-limits'>
                <div>
                    <span className='lending-stat-label'>Max loan</span>
                    <span className='dash-limit-value'>{pesosCompact(band.cap)}</span>
                </div>
                <div>
                    <span className='lending-stat-label'>Secured</span>
                    <span className='dash-limit-value'>{rate(band.secured_bps)}</span>
                </div>
                <div>
                    <span className='lending-stat-label'>Guarantor</span>
                    <span className='dash-limit-value'>{rate(band.guarantor_bps)}</span>
                </div>
            </div>
        </section>
    )
}

export default TierLimitsCard
