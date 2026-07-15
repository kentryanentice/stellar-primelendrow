import { ShieldCheck } from 'lucide-react'
import { CREDIT_SCORE_MAX } from '../../functions/useCreditScore'
import { pesos, rate } from '../../functions/Lending/money'
import type { PoolResponse } from '../../functions/Lending/types'
import type { BorrowFormState } from '../../functions/Lending/useBorrowForm'

/**
 * The caller's live eligibility for whichever product is selected in the
 * apply form next to it: score against the band ladder, tier, and — once a
 * quote has come back — that product's rate and cap, verbatim from the
 * engine (same `form` the BorrowCard renders, so the two never disagree).
 */
function EligibilityCard({ data, form }: { data: PoolResponse; form: BorrowFormState }) {
    const { me, params } = data
    const bands = params.policy.bands
    const tierIndex = bands.findIndex(b => me.score >= b.min_score && me.score <= b.max_score)
    const band = tierIndex >= 0 ? bands[tierIndex] : null
    const { productQuote } = form

    return (
        <section className='lending-card lending-card-eligibility'>
            <div className='lending-card-head'>
                <span className='lending-card-icon is-accent'><ShieldCheck /></span>
                <h2>Your eligibility</h2>
                {band && <span className='lending-pill is-accent'>Tier {tierIndex + 1}</span>}
            </div>

            <div className='lending-eligibility-score'>
                <span className='lending-eligibility-score-value'>{me.score}</span>
                <span className='lending-eligibility-score-max'>
                    / {CREDIT_SCORE_MAX}{band && <> · band {band.min_score}–{band.max_score}</>}
                </span>
            </div>
            <div className='lending-utilization-track'>
                <div className='lending-utilization-fill' style={{ width: `${Math.min(100, (me.score / CREDIT_SCORE_MAX) * 100)}%` }} />
            </div>

            {/* no .lending-quote box here — the whole card already carries the
                accent treatment, so a second nested box would double up */}
            <div className='lending-eligibility-rows'>
                <div className='lending-quote-row'>
                    <span>Rate for this product</span>
                    <b>{productQuote ? rate(productQuote.rate_bps) : '—'}</b>
                </div>
                <div className='lending-quote-row'>
                    <span>Max you can borrow</span>
                    <b>{productQuote ? pesos(productQuote.max_amount) : '—'}</b>
                </div>
            </div>
        </section>
    )
}

export default EligibilityCard
