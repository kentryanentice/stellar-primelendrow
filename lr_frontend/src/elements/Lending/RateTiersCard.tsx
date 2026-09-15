import { useState } from 'react'
import { ChevronDown, ChevronUp } from 'lucide-react'
import { pesosCompact, rate } from '../../functions/Lending/money'
import type { PoolResponse } from '../../functions/Lending/types'

/**
 * The engine's live rate card (the policy bands) — the same data the engine
 * prices loans with, not a copy that can drift. Collapsed to just the
 * caller's own tier by default; "Compare tiers" expands the full table with
 * the caller's row highlighted, same as before.
 *
 * Deposit limits (engine 042) sit alongside: the caller's own maximum single
 * deposit in the summary row, and the full deposit-limit table when expanded.
 * They are their own table because their score cutoffs can differ from the
 * pricing bands. The summary figure is the engine's number for this member —
 * including the reduced limits below the lowest tier — never worked out here.
 */
function RateTiersCard({ data }: { data: PoolResponse }) {
    const { me, params } = data
    const bands = params.policy.bands
    const myBand = bands.find(b => me.score >= b.min_score && me.score <= b.max_score) ?? null
    // `?? null` so an engine older than the page degrades to "not shown"
    // instead of crashing the card.
    const myDeposit = me.deposit_limits ?? null
    const depositTiers = params.policy.deposit_limits?.tiers ?? []
    const lowestDeposit = depositTiers[0] ?? null
    const belowDepositTiers = lowestDeposit !== null && me.score < lowestDeposit.min_score
    const [open, setOpen] = useState(false)

    return (
        <section className='lending-card lending-card-rates'>
            <div className='lending-tier-summary'>
                <div className='lending-tier-summary-score'>
                    <span className='lending-stat-label'>Your tier</span>
                    <span className='lending-tier-summary-band'>Score {myBand ? `${myBand.min_score}–${myBand.max_score}` : me.score}</span>
                </div>
                {(myBand || myDeposit) && (
                    <>
                        <div className='lending-tier-summary-divider' />
                        <div className='lending-tier-summary-stats'>
                            {myBand ? (
                                <>
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
                                </>
                            ) : (
                                <div>
                                    <span className='lending-stat-label'>Max loan</span>
                                    <span className='lending-stat-value'>Not yet</span>
                                </div>
                            )}
                            {myDeposit && (
                                <div>
                                    <span className='lending-stat-label'>Max deposit</span>
                                    <span className='lending-stat-value'>{pesosCompact(myDeposit.limits.per_deposit)}</span>
                                </div>
                            )}
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

                    {depositTiers.length > 0 && lowestDeposit && (
                        <>
                            <p className='lending-rates-title'>Deposit limits</p>
                            <div className='lending-rates-scroll'>
                                <table className='lending-rates-table'>
                                    <thead>
                                        <tr>
                                            <th>Score</th>
                                            <th>Per deposit</th>
                                            <th>24 hours</th>
                                            <th>30 days</th>
                                            <th>Balance cap</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {lowestDeposit.min_score > 0 && (
                                            <tr className={belowDepositTiers ? 'is-mine' : undefined}>
                                                <td>
                                                    Below {lowestDeposit.min_score}
                                                    {belowDepositTiers && <span className='lending-pill is-accent lending-you-badge'>You</span>}
                                                </td>
                                                <td colSpan={4} className='lending-muted'>
                                                    {params.policy.deposit_limits.below_floor_pct}% of the {lowestDeposit.min_score}–{lowestDeposit.max_score} limits
                                                </td>
                                            </tr>
                                        )}
                                        {depositTiers.map(tier => {
                                            const mine = me.score >= tier.min_score && me.score <= tier.max_score
                                            return (
                                                <tr key={tier.min_score} className={mine ? 'is-mine' : undefined}>
                                                    <td>
                                                        {tier.min_score}–{tier.max_score}
                                                        {mine && <span className='lending-pill is-accent lending-you-badge'>You</span>}
                                                    </td>
                                                    <td>{pesosCompact(tier.per_deposit)}</td>
                                                    <td>{pesosCompact(tier.daily)}</td>
                                                    <td>{pesosCompact(tier.monthly)}</td>
                                                    <td>{pesosCompact(tier.max_balance)}</td>
                                                </tr>
                                            )
                                        })}
                                    </tbody>
                                </table>
                            </div>
                            <p className='lending-muted'>
                                Deposits count toward the 24-hour and 30-day limits even if you withdraw them. Interest you
                                earn doesn’t count and can take your balance past the cap.
                            </p>
                        </>
                    )}
                </>
            )}
        </section>
    )
}

export default RateTiersCard
