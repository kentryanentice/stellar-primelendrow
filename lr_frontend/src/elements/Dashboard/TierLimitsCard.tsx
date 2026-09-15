import { pesosCompact, rate } from '../../functions/Lending/money'
import type { PoolResponse } from '../../functions/Lending/types'

/**
 * The caller's own tier, as four figures — the compact form of RateTiersCard's
 * summary row, sized for a narrow column. The full ladders stay on the Lend
 * page.
 *
 * Renders for every member, including one scored below every pricing band:
 * they can't borrow, but their deposit limits still apply (reduced, engine
 * 042), and that is exactly when they most need to see them.
 */
function TierLimitsCard({ data }: { data: PoolResponse }) {
    const { me, params } = data
    const band = params.policy.bands.find(b => me.score >= b.min_score && me.score <= b.max_score) ?? null
    const deposit = me.deposit_limits ?? null
    if (!band && !deposit) return null

    return (
        <section className='lending-card'>
            <span className='lending-stat-label'>Your tier limits</span>
            <div className='dash-limits'>
                <div>
                    <span className='lending-stat-label'>Max loan</span>
                    <span className='dash-limit-value'>{band ? pesosCompact(band.cap) : 'Not yet'}</span>
                </div>
                <div>
                    <span className='lending-stat-label'>Max deposit</span>
                    <span className='dash-limit-value'>{deposit ? pesosCompact(deposit.limits.per_deposit) : '—'}</span>
                </div>
                <div>
                    <span className='lending-stat-label'>Secured</span>
                    <span className='dash-limit-value'>{band ? rate(band.secured_bps) : '—'}</span>
                </div>
                <div>
                    <span className='lending-stat-label'>Guarantor</span>
                    <span className='dash-limit-value'>{band ? rate(band.guarantor_bps) : '—'}</span>
                </div>
            </div>
            {!band && (
                <p className='lending-muted'>Your score is below every borrowing tier, so deposit limits are reduced.</p>
            )}
        </section>
    )
}

export default TierLimitsCard
