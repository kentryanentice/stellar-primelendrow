import { useState } from 'react'
import { ChartPie, ChevronDown, ChevronUp } from 'lucide-react'
import { pesos } from '../../functions/Lending/money'
import { RECIPIENTS, widthPct, type Recipient } from '../../functions/Lending/split'
import type { InterestParts, PoolResponse } from '../../functions/Lending/types'

type Scenario = { id: string; tab: string; column: string; parts: InterestParts }

/**
 * The published interest split (SOW deliverable 3) as a picture: one bar for
 * where a single interest payment goes, switchable between "no guarantor"
 * and each guarantor score tier, plus the full worked-example table on
 * demand. The amounts are the engine's own split of its example payment
 * (GET /pool → params.split_example); this card draws them and adds nothing.
 */
function InterestSplitCard({ data }: { data: PoolResponse }) {
    const { split_example: example, policy } = data.params
    const scenarios: Scenario[] = [
        { id: 'none', tab: 'No guarantor', column: 'No guarantor', parts: example.no_guarantor },
        ...example.tiers.map(t => ({
            id: `${t.min_score}`,
            tab: `${t.min_score}–${t.max_score}`,
            column: `Guarantor @ ${t.share}%`,
            parts: t.parts,
        })),
    ]

    const [selectedId, setSelectedId] = useState('none')
    const [hovered, setHovered] = useState<Recipient | null>(null)
    const [open, setOpen] = useState(false)
    const selected = scenarios.find(s => s.id === selectedId) ?? scenarios[0]
    const drawn = RECIPIENTS.filter(r => selected.parts[r.key] > 0)

    return (
        <section className='lending-card lending-card-split'>
            <div className='lending-card-head'>
                <span className='lending-card-icon is-accent'><ChartPie /></span>
                <h2>Where interest goes</h2>
                <span className='lending-pill is-accent'>{pesos(example.interest)} example</span>
            </div>

            <p className='lending-muted'>
                Every interest payment is split by one published rule, to the exact centavo. Here is how{' '}
                <b>{pesos(example.interest)}</b> of interest is divided.
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

            <div className='lending-split-bar' role='img' aria-label={drawn.map(r => `${r.label} ${pesos(selected.parts[r.key])}`).join(', ')}>
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
                                <span>{r.label}</span>
                                <b>{pesos(selected.parts[r.key])}</b>
                            </span>
                        )}
                    </span>
                ))}
            </div>

            <ul className='lending-split-legend'>
                {RECIPIENTS.map(r => (
                    <li
                        key={r.key}
                        className={hovered === r.key ? 'is-hovered' : undefined}
                        onMouseEnter={() => setHovered(r.key)}
                        onMouseLeave={() => setHovered(null)}
                    >
                        <span className={`lending-split-swatch is-${r.key}`} aria-hidden='true' />
                        <span className='lending-split-legend-text'>
                            <b>{r.label}</b>
                            <span>{r.blurb(policy.interest_split)}</span>
                        </span>
                        <span className='lending-split-legend-amount'>{pesos(selected.parts[r.key])}</span>
                    </li>
                ))}
                <li className='lending-split-legend-total'>
                    <span className='lending-split-legend-text'><b>Total</b></span>
                    <span className='lending-split-legend-amount'>{pesos(example.interest)}</span>
                </li>
            </ul>

            <button type='button' className='lending-btn lending-tier-toggle' onClick={() => setOpen(o => !o)}>
                {open ? 'Hide comparison' : 'Compare every tier'} {open ? <ChevronUp /> : <ChevronDown />}
            </button>

            {open && (
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
                                <tr key={r.key}>
                                    <td>
                                        <span className={`lending-split-swatch is-${r.key}`} aria-hidden='true' />
                                        {r.label}
                                    </td>
                                    {scenarios.map(s => (
                                        <td key={s.id} className={s.id === selected.id ? 'is-mine' : undefined}>
                                            {pesos(s.parts[r.key])}
                                        </td>
                                    ))}
                                </tr>
                            ))}
                            <tr className='lending-split-table-total'>
                                <td>Total</td>
                                {scenarios.map(s => (
                                    <td key={s.id} className={s.id === selected.id ? 'is-mine' : undefined}>
                                        {pesos(example.interest)}
                                    </td>
                                ))}
                            </tr>
                        </tbody>
                    </table>
                </div>
            )}
        </section>
    )
}

export default InterestSplitCard
