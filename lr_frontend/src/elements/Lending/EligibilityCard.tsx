import { CREDIT_SCORE_MAX } from '../../functions/useCreditScore'
import { pesosCompact, rate } from '../../functions/Lending/money'
import type { PoolResponse } from '../../functions/Lending/types'

/**
 * The caller's credit standing: score against the band ladder, tier, and —
 * if there's a next tier to reach — what repaying on time unlocks. The
 * product-specific rate/cap preview used to be duplicated here too, but
 * that's exactly what BorrowCard's own quote tiles already show, so it
 * isn't repeated.
 */
function EligibilityCard({ data }: { data: PoolResponse }) {
    const { me, params } = data
    const bands = params.policy.bands
    const tierIndex = bands.findIndex(b => me.score >= b.min_score && me.score <= b.max_score)
    const band = tierIndex >= 0 ? bands[tierIndex] : null
    const nextBand = tierIndex >= 0 && tierIndex + 1 < bands.length ? bands[tierIndex + 1] : null

    return (
        <section className='lending-card lending-card-eligibility'>
            <div className='lending-eligibility-head'>
                <div>
                    <span className='lending-stat-label'>Credit score</span>
                    <div className='lending-eligibility-score'>
                        <span className='lending-eligibility-score-value'>{me.score}</span>
                        <span className='lending-eligibility-score-max'>/ {CREDIT_SCORE_MAX}</span>
                    </div>
                </div>
                {band && <span className='lending-pill is-accent'>Tier {tierIndex + 1} · Band {band.min_score}–{band.max_score}</span>}
            </div>
            <div className='lending-utilization-track'>
                <div className='lending-utilization-fill' style={{ width: `${Math.min(100, (me.score / CREDIT_SCORE_MAX) * 100)}%` }} />
            </div>
            {nextBand && (
                <p className='lending-muted'>
                    Repay on time to reach band {nextBand.min_score}–{nextBand.max_score}: {pesosCompact(nextBand.cap)} cap at {rate(nextBand.secured_bps)}.
                </p>
            )}
        </section>
    )
}

export default EligibilityCard
