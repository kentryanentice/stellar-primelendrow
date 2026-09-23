import { Link } from 'react-router-dom'
import { Gauge } from 'lucide-react'
import { formatDate } from '../../functions/Lending/money'
import { CREDIT_SCORE_MAX } from '../../functions/useCreditScore'
import { useCreditHistory, type ScoreAtStake, type ScoreChange, type UpcomingRise } from '../../functions/useCreditHistory'
import { SCORE_REASON_LABEL, loanRef, points } from '../Records/recordLabels'
import { ActivityRowsSkeleton } from './DashboardSkeleton'

/** Each way an open loan can still end, said from the member's side of it. */
const IF_LABEL: Record<string, string> = {
    loan_repaid_term_complete: 'You repay it in full — the rise lands once the term ends',
    loan_defaulted: 'You stop paying and the loan is declared in default',
    default_settled: 'You settle the default',
    guarantor_claimed: 'The borrower stops paying and your pledge is claimed',
    guarantor_claim_settled: 'The borrower settles the default',
}

const STATUS_PILL: Record<string, { label: string; cls: string }> = {
    defaulted: { label: 'In default', cls: 'is-warn' },
    reconciling: { label: 'Settling', cls: 'is-progress' },
}

const tone = (delta: number) => (delta > 0 ? 'is-good' : delta < 0 ? 'is-warn' : '')

/** The loan's public record, where every one of these changes is shown too. */
function LoanLink({ id }: { id: string }) {
    return <Link to={`/records/${id}`} className='lending-inline-link'>#{loanRef(id)}</Link>
}

/** Why a past change happened. Rows from before reason codes keep the
 *  sentence they were logged with. */
function changeReason(change: ScoreChange) {
    if (change.reason) return SCORE_REASON_LABEL[change.reason] ?? change.reason
    if (change.score_from === null) return 'Account opened'
    return change.note ?? 'Score changed'
}

function Upcoming({ rows }: { rows: UpcomingRise[] }) {
    return (
        <div>
            <span className='lending-stat-label'>Coming up</span>
            <p className='lending-muted'>
                Earned by paying a loan off. Each rise lands when that loan’s term ends; the expected score assumes
                nothing else changes first.
            </p>
            <div className='lending-rates-scroll'>
                <table className='lending-rates-table'>
                    <thead>
                        <tr><th>Loan</th><th>Change</th><th>Score</th><th>Lands</th></tr>
                    </thead>
                    <tbody>
                        {rows.map(rise => (
                            <tr key={rise.loan_id}>
                                <td>
                                    <LoanLink id={rise.loan_id} />
                                    {rise.paid_off_at !== null && (
                                        <><br /><span className='lending-muted'>Paid off {formatDate(rise.paid_off_at)}</span></>
                                    )}
                                </td>
                                <td><span className='lending-tx-status is-progress'>{points(rise.score_to - rise.score_from)}</span></td>
                                <td>
                                    {rise.score_from} → {rise.score_to}
                                    <br />
                                    <span className='lending-muted'>Expected</span>
                                </td>
                                <td>
                                    {formatDate(rise.due_at)}
                                    <br />
                                    <span className='lending-muted'>
                                        {rise.term_end !== null && rise.due_at > rise.term_end ? 'Spaced after an earlier rise' : 'Term end'}
                                    </span>
                                </td>
                            </tr>
                        ))}
                    </tbody>
                </table>
            </div>
        </div>
    )
}

function AtStake({ rows, score }: { rows: ScoreAtStake[]; score: number }) {
    return (
        <div>
            <span className='lending-stat-label'>What could still change</span>
            <p className='lending-muted'>
                Each outcome from today’s score of <b>{score}</b>, one at a time. A missed payment costs no points by
                itself — the deduction comes only if the loan is declared in default.
            </p>
            {rows.length === 0 ? (
                <p className='lending-muted'>No open loans or pledges — nothing can move your score right now.</p>
            ) : (
                <div className='lending-rates-scroll'>
                    <table className='lending-rates-table'>
                        <thead>
                            <tr><th>Loan</th><th>If</th><th>Change</th><th>Score</th></tr>
                        </thead>
                        <tbody>
                            {rows.flatMap(stake => stake.outcomes.map((outcome, i) => (
                                <tr key={`${stake.loan_id}-${stake.role}-${outcome.reason}`}>
                                    {i === 0 && (
                                        <td rowSpan={stake.outcomes.length}>
                                            <LoanLink id={stake.loan_id} />
                                            <br />
                                            <span className='lending-muted'>{stake.role === 'borrower' ? 'You borrowed' : 'You guarantee'}</span>
                                            {STATUS_PILL[stake.status] && (
                                                <>
                                                    <br />
                                                    <span className={`lending-tx-status ${STATUS_PILL[stake.status].cls}`}>
                                                        {STATUS_PILL[stake.status].label}
                                                    </span>
                                                </>
                                            )}
                                            {stake.overdue_since !== null && (
                                                <>
                                                    <br />
                                                    <span className='lending-tx-status is-warn'>Overdue since {formatDate(stake.overdue_since)}</span>
                                                </>
                                            )}
                                        </td>
                                    )}
                                    <td>
                                        {IF_LABEL[outcome.reason] ?? outcome.reason}
                                        {outcome.reason === 'loan_repaid_term_complete' && stake.term_end !== null && (
                                            <><br /><span className='lending-muted'>Term ends {formatDate(stake.term_end)}</span></>
                                        )}
                                        {outcome.reason === 'loan_defaulted' && stake.guarantors > 0 && (
                                            <>
                                                <br />
                                                <span className='lending-muted'>
                                                    {stake.guarantors === 1 ? 'Your guarantor' : `Your ${stake.guarantors} guarantors`} could
                                                    also lose points if their pledge is claimed.
                                                </span>
                                            </>
                                        )}
                                    </td>
                                    <td><span className={`lending-tx-status ${tone(outcome.delta)}`}>{points(outcome.delta)}</span></td>
                                    <td>{score} → {outcome.score_to}</td>
                                </tr>
                            )))}
                        </tbody>
                    </table>
                </div>
            )}
        </div>
    )
}

function History({ rows }: { rows: ScoreChange[] }) {
    return (
        <div>
            <span className='lending-stat-label'>History</span>
            <div className='lending-rates-scroll'>
                <table className='lending-rates-table'>
                    <thead>
                        <tr><th>Date</th><th>Change</th><th>Score</th><th>Why</th><th>Loan</th></tr>
                    </thead>
                    <tbody>
                        {rows.map((change, i) => (
                            <tr key={`${change.at}-${i}`}>
                                <td>{formatDate(change.at)}</td>
                                <td>
                                    {change.score_from === null ? '—' : (
                                        <span className={`lending-tx-status ${tone(change.score_to - change.score_from)}`}>
                                            {points(change.score_to - change.score_from)}
                                        </span>
                                    )}
                                </td>
                                <td>{change.score_from === null ? change.score_to : `${change.score_from} → ${change.score_to}`}</td>
                                <td>{changeReason(change)}</td>
                                <td>{change.loan_id ? <LoanLink id={change.loan_id} /> : '—'}</td>
                            </tr>
                        ))}
                    </tbody>
                </table>
            </div>
        </div>
    )
}

/**
 * The member's credit score, explained: rises already earned and waiting on a
 * term, what each open loan or pledge could still do — deductions included,
 * with the score each would leave — and every change so far, from and to.
 * Each loan links to its public record, which shows the same changes.
 */
function CreditHistoryCard() {
    const { data, loading, error } = useCreditHistory()

    return (
        <section className='lending-card'>
            <div className='lending-card-head'>
                <span className='lending-card-icon is-accent'><Gauge /></span>
                <h2>Credit score</h2>
                {data && <span className='lending-muted'>Now <b>{data.score}</b> / {CREDIT_SCORE_MAX}</span>}
            </div>

            {loading ? (
                <ActivityRowsSkeleton rows={3} />
            ) : error || !data ? (
                <p className='lending-muted'>Couldn’t load your score history. Please try again later.</p>
            ) : (
                <>
                    {data.upcoming.length > 0 && <Upcoming rows={data.upcoming} />}
                    <AtStake rows={data.at_stake} score={data.score} />
                    <History rows={data.changes} />
                </>
            )}
        </section>
    )
}

export default CreditHistoryCard
