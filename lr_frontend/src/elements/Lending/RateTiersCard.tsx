import { List } from 'lucide-react'
import { pesosCompact, rate } from '../../functions/Lending/money'
import type { PoolResponse } from '../../functions/Lending/types'

/**
 * The engine's live rate card (the policy bands) — the same data the engine
 * prices loans with, not a copy that can drift. The caller's own tier is
 * highlighted with a "You" pill so the applicable row reads at a glance.
 */
function RateTiersCard({ data }: { data: PoolResponse }) {
    const { me, params } = data
    const bands = params.policy.bands

    return (
        <section className='lending-card lending-card-rates'>
            <div className='lending-card-head'>
                <span className='lending-card-icon is-accent'><List /></span>
                <h2>Rate & limit tiers</h2>
            </div>

            <p className='lending-muted'>Higher credit scores unlock larger loans at lower monthly rates. Your tier is highlighted.</p>

            <div className='lending-rates-scroll'>
                <table className='lending-rates-table'>
                    <thead>
                        <tr>
                            <th>Score</th>
                            <th>Max loan</th>
                            <th>Secured</th>
                            <th>Guarantor</th>
                        </tr>
                    </thead>
                    <tbody>
                        {bands.map(band => {
                            const mine = me.score >= band.min_score && me.score <= band.max_score
                            return (
                                <tr key={band.min_score} className={mine ? 'is-mine' : undefined}>
                                    <td>
                                        {band.min_score}–{band.max_score}
                                        {mine && <span className='lending-pill is-accent lending-you-badge'>You</span>}
                                    </td>
                                    <td>{pesosCompact(band.cap)}</td>
                                    <td>{rate(band.secured_bps)}</td>
                                    <td>{rate(band.guarantor_bps)}</td>
                                </tr>
                            )
                        })}
                    </tbody>
                </table>
            </div>

            <p className='lending-muted'>
                Deposit-backed and XLM-collateral loans use the secured rate; guarantor backing doubles your cap
                (up to ×{params.policy.guarantor_cap_multiple}) at the guarantor rate.
            </p>
        </section>
    )
}

export default RateTiersCard
