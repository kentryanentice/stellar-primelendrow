import { useState } from 'react'
import { ChevronDown, ChevronUp } from 'lucide-react'
import { pesosCompact, rate } from '../../functions/Lending/money'
import type { PoolResponse } from '../../functions/Lending/types'

/**
 * The engine's live rate card (the policy bands) — the same data the engine
 * prices loans with, not a copy that can drift. Collapsed to just the
 * caller's own tier by default; "Compare tiers" expands the full table with
 * the caller's row highlighted, same as before.
 */
function RateTiersCard({ data }: { data: PoolResponse }) {
    const { me, params } = data
    const bands = params.policy.bands
    const myBand = bands.find(b => me.score >= b.min_score && me.score <= b.max_score) ?? null
    const [open, setOpen] = useState(false)

    return (
        <section className='lending-card lending-card-rates'>
            <div className='lending-tier-summary'>
                <div className='lending-tier-summary-score'>
                    <span className='lending-stat-label'>Your tier</span>
                    <span className='lending-tier-summary-band'>Score {myBand ? `${myBand.min_score}–${myBand.max_score}` : me.score}</span>
                </div>
                {myBand && (
                    <>
                        <div className='lending-tier-summary-divider' />
                        <div className='lending-tier-summary-stats'>
                            <div>
                                <span className='lending-stat-label'>Max loan</span>
                                <span className='lending-stat-value'>{pesosCompact(myBand.cap)}</span>
                            </div>
                            <div>
                                <span className='lending-stat-label'>Secured</span>
                                <span className='lending-stat-value'>{rate(myBand.secured_bps)}</span>
                            </div>
                            <div>
                                <span className='lending-stat-label'>Guarantor</span>
                                <span className='lending-stat-value'>{rate(myBand.guarantor_bps)}</span>
                            </div>
                        </div>
                    </>
                )}
                <button type='button' className='lending-btn lending-tier-toggle' onClick={() => setOpen(o => !o)}>
                    {open ? 'Hide tiers' : 'Compare tiers'} {open ? <ChevronUp /> : <ChevronDown />}
                </button>
            </div>

            {open && (
                <>
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
                                    const mine = band === myBand
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
                </>
            )}
        </section>
    )
}

export default RateTiersCard
