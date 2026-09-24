import { useState } from 'react'
import { ChevronDown, ChevronUp } from 'lucide-react'
import { pesos, pesosCompact } from '../../functions/Lending/money'
import { RECIPIENTS, widthPct, type Recipient } from '../../functions/Lending/split'
import type { InterestParts, PoolResponse } from '../../functions/Lending/types'

type Scenario = { id: string; tab: string; column: string; parts: InterestParts }

/**
 * The interest split (SOW deliverable 3), laid out like RateTiersCard.
 * Collapsed, it is the real thing: every peso of interest the pool has
 * collected and where each recorded split actually sent it (pool.interest),
 * with the caller's own share of it alongside (me.interest_earned).
 * "How it's split" expands the published rule behind it — a bar switchable
 * across the guarantor score tiers plus the full worked-example table
 * (params.split_example). Both are the engine's numbers; this card only
 * draws them.
 */
function InterestSplitCard({ data }: { data: PoolResponse }) {
    const { split_example: example, policy } = data.params
    const collected = data.pool.interest
    const mine = data.me.interest_earned
    /** The caller's own deposits against the whole pool — what their slice of
     *  the depositors' share is proportional to. Their own loan proceeds are
     *  left out, as the engine leaves them out of that share. Display only. */
    const myBalance = data.me.available - data.me.proceeds + data.me.lent + data.me.collateral + data.me.pledged
    const poolShare = data.pool.total_deposits > 0
        ? `${((myBalance / data.pool.total_deposits) * 100).toFixed(2).replace(/\.?0+$/, '')}%`
        : null
    const split = policy.interest_split
    const scenarios: Scenario[] = [
        { id: 'none', tab: 'No guarantor', column: 'No guarantor', parts: example.no_guarantor },
        ...example.tiers.map(t => ({
            id: `${t.min_score}`,
            tab: `${t.min_score}–${t.max_score}`,
            column: `Guarantor @ ${t.share}%`,
            parts: t.parts,
        })),
    ]

    const [open, setOpen] = useState(false)
    const [selectedId, setSelectedId] = useState('none')
    const [hovered, setHovered] = useState<Recipient | null>(null)
    const selected = scenarios.find(s => s.id === selectedId) ?? scenarios[0]
    const drawn = RECIPIENTS.filter(r => selected.parts[r.key] > 0)
    const collectedDrawn = RECIPIENTS.filter(r => collected.parts[r.key] > 0)
    /** A part as a share of the payment it came out of — the engine's amounts,
     *  said a second way. Drawing only, never a figure sent back. */
    const sharePct = (part: number, whole: number) =>
        whole > 0 ? `${((part / whole) * 100).toFixed(1).replace(/\.0$/, '')}%` : '—'

    return (
        <section className='lending-card lending-card-split'>
            <div className='lending-tier-summary'>
                <div className='lending-tier-summary-score'>
                    <span className='lending-stat-label'>Interest split</span>
                    <span className='lending-tier-summary-band'>
                        {collected.payments} {collected.payments === 1 ? 'payment' : 'payments'}
                    </span>
                </div>
                <div className='lending-tier-summary-divider' />
                <div className='lending-tier-summary-stats'>
                    {RECIPIENTS.map(r => (
                        <div key={r.key}>
                            <span className='lending-stat-label'>
                                <span className={`lending-split-swatch is-${r.key}`} aria-hidden='true' />
                                {r.label}
                            </span>
                            <span className='lending-stat-value'>
                                {pesosCompact(collected.parts[r.key])}
                                {/* What that is as a share of the interest
                                    collected so far — the fixed shares read
                                    10/20/40%, the band's two split by tier. */}
                                {collected.total > 0 && (
                                    <span className='lending-split-pct'>{sharePct(collected.parts[r.key], collected.total)}</span>
                                )}
                            </span>
                        </div>
                    ))}
                </div>
                <div className='lending-tier-summary-divider' />
                {/* The caller's own slice of all that, next to the pool's. */}
                <div className='lending-tier-summary-score lending-split-mine'>
                    <span className='lending-stat-label'>Your share</span>
                    <span className='lending-stat-value is-good'>{pesos(mine.total)}</span>
                    <span className='lending-muted'>
                        {mine.as_guarantor > 0
                            ? `${pesos(mine.as_depositor)} as depositor · ${pesos(mine.as_guarantor)} as guarantor`
                            : poolShare
                                ? `You hold ${poolShare} of the pool`
                                : 'Deposit to earn a share'}
                    </span>
                </div>
                <button type='button' className='lending-btn lending-tier-toggle' onClick={() => setOpen(o => !o)}>
                    {open ? 'Hide rule' : 'How it’s split'} {open ? <ChevronUp /> : <ChevronDown />}
                </button>
            </div>

            {collected.total > 0 ? (
                <div
                    className='lending-split-bar is-thin'
                    role='img'
                    aria-label={`Collected ${pesos(collected.total)}: ${collectedDrawn.map(r => `${r.label} ${pesos(collected.parts[r.key])}`).join(', ')}`}
                >
                    {collectedDrawn.map(r => (
                        <span
                            key={r.key}
                            className={`lending-split-seg is-${r.key}`}
                            style={{ width: `${widthPct(collected.parts[r.key], collected.total)}%` }}
                        />
                    ))}
                </div>
            ) : (
                <p className='lending-muted'>No interest collected yet. Each repayment’s split shows up here as it lands.</p>
            )}

            {open && (
                <>
                    <p className='lending-rates-title'>
                        The published rule, on <i>{pesos(example.interest)}</i> of interest
                    </p>
                    <div className='lending-split-scenarios'>
                        <span className='lending-stat-label'>Guarantor, by score tier</span>
                        <div className='lending-tab-group' role='tablist'>
                            {scenarios.map(s => (
                                <button
                                    key={s.id}
                                    type='button'
                                    role='tab'
                                    aria-selected={s.id === selected.id}
                                    className={`lending-tab${s.id === selected.id ? ' is-active' : ''}`}
                                    onClick={() => setSelectedId(s.id)}
                                >
                                    {s.tab}
                                </button>
                            ))}
                        </div>
                    </div>

                    <div
                        className='lending-split-bar'
                        role='img'
                        aria-label={drawn.map(r => `${r.label} ${pesos(selected.parts[r.key])}`).join(', ')}
                    >
                        {drawn.map(r => (
                            <span
                                key={r.key}
                                className={`lending-split-seg is-${r.key}${hovered && hovered !== r.key ? ' is-dimmed' : ''}`}
                                style={{ width: `${widthPct(selected.parts[r.key], example.interest)}%` }}
                                tabIndex={0}
                                onMouseEnter={() => setHovered(r.key)}
                                onMouseLeave={() => setHovered(null)}
                                onFocus={() => setHovered(r.key)}
                                onBlur={() => setHovered(null)}
                            >
                                {hovered === r.key && (
                                    <span className='lending-split-tip' role='tooltip'>
                                        <span>{r.label} · {sharePct(selected.parts[r.key], example.interest)}</span>
                                        <b>{pesos(selected.parts[r.key])}</b>
                                    </span>
                                )}
                            </span>
                        ))}
                    </div>

                    <div className='lending-rates-scroll'>
                        <table className='lending-rates-table lending-split-table'>
                            <thead>
                                <tr>
                                    <th>Recipient</th>
                                    {scenarios.map(s => (
                                        <th key={s.id} className={s.id === selected.id ? 'is-mine' : undefined}>{s.column}</th>
                                    ))}
                                </tr>
                            </thead>
                            <tbody>
                                {RECIPIENTS.map(r => (
                                    <tr
                                        key={r.key}
                                        className={hovered === r.key ? 'is-hovered' : undefined}
                                        onMouseEnter={() => setHovered(r.key)}
                                        onMouseLeave={() => setHovered(null)}
                                    >
                                        <td>
                                            <span className={`lending-split-swatch is-${r.key}`} aria-hidden='true' />
                                            {r.label}
                                        </td>
                                        {scenarios.map(s => (
                                            <td key={s.id} className={s.id === selected.id ? 'is-mine' : undefined}>
                                                {pesos(s.parts[r.key])}
                                                <span className='lending-split-pct'>{sharePct(s.parts[r.key], example.interest)}</span>
                                            </td>
                                        ))}
                                    </tr>
                                ))}
                                <tr className='lending-split-table-total'>
                                    <td>Total</td>
                                    {scenarios.map(s => (
                                        <td key={s.id} className={s.id === selected.id ? 'is-mine' : undefined}>
                                            {pesos(example.interest)}
                                            <span className='lending-split-pct'>100%</span>
                                        </td>
                                    ))}
                                </tr>
                            </tbody>
                        </table>
                    </div>

                    <p className='lending-muted'>
                        Depositors ({split.depositors}%, shared in proportion to deposits), the lending reserve
                        ({split.reserve}%) and the platform fee ({split.platform}%) are fixed. Guarantors are paid from
                        the {split.risk_band}% risk band by score tier, capped at {split.guarantor_cap}%, and the
                        recovery fund keeps the rest of the band to absorb defaults.
                    </p>
                </>
            )}
        </section>
    )
}

export default InterestSplitCard
