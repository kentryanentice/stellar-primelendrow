import { AlertTriangle, ExternalLink, Landmark, Receipt, ShieldCheck, Split } from 'lucide-react'
import { formatDate, pesos, rate, xlm } from '../../functions/Lending/money'
import { shortId, txLink } from '../../functions/Lending/explorer'
import { RECIPIENTS, widthPct } from '../../functions/Lending/split'
import { PRODUCT_LABEL } from '../../functions/Lending/types'
import { PUBLIC_STATUS_LABEL, type PublicLoanDetail, type PublicPayment } from '../../functions/Lending/publicLoans'
import {
    COLLATERAL_STATUS_LABEL,
    INSTALLMENT_STATUS_LABEL,
    PLEDGE_STATUS_LABEL,
    STATUS_CLS,
    VAULT_ACTION_LABEL,
    loanRef,
    recoverySource,
    shareOf,
} from './recordLabels'

function TxLink({ hash }: { hash: string }) {
    return (
        <a className='lending-inline-link' href={txLink(hash)} target='_blank' rel='noreferrer noopener'>
            <span>{shortId(hash)}</span>
            <ExternalLink aria-hidden='true' />
        </a>
    )
}

/** The loan's life as dated steps — only the ones that happened. */
function timeline({ loan }: PublicLoanDetail) {
    const steps: { label: string; at: number; tone?: 'bad' | 'good' }[] = [{ label: 'Applied', at: loan.applied_at }]
    if (loan.disbursed_at !== null) steps.push({ label: 'Disbursed', at: loan.disbursed_at })
    if (loan.status === 'declined') steps.push({ label: 'Declined', at: loan.updated_at, tone: 'bad' })
    if (loan.status === 'cancelled') steps.push({ label: 'Cancelled', at: loan.updated_at, tone: 'bad' })
    if (loan.defaulted_at !== null) steps.push({ label: 'Defaulted', at: loan.defaulted_at, tone: 'bad' })
    if (loan.reconciled_at !== null) steps.push({ label: 'Settled after default', at: loan.reconciled_at, tone: 'good' })
    if (loan.closed_at !== null) steps.push({ label: 'Repaid in full', at: loan.closed_at, tone: 'good' })
    return steps.sort((a, b) => a.at - b.at)
}

/** One repayment's interest, line by line. The lines add back to `interest`. */
function SplitTable({ payment }: { payment: PublicPayment }) {
    const split = payment.split
    if (!split) {
        return (
            <p className='lending-muted'>
                {payment.interest_paid > 0 ? 'Booked before per-payment splits were recorded.' : 'No interest in this payment.'}
            </p>
        )
    }
    const { interest, parts } = split
    const drawn = RECIPIENTS.filter(r => parts[r.key] > 0)
    return (
        <>
            <div className='lending-split-bar is-thin' aria-hidden='true'>
                {drawn.map(r => (
                    <span
                        key={r.key}
                        className={`lending-split-seg is-${r.key}`}
                        style={{ width: `${widthPct(parts[r.key], interest)}%` }}
                    />
                ))}
            </div>
            <div className='lending-rates-scroll'>
                <table className='lending-rates-table'>
                    <thead>
                        <tr><th>Went to</th><th>Why</th><th>Share</th><th>Amount</th></tr>
                    </thead>
                    <tbody>
                        <tr>
                            <td><span className='lending-split-swatch is-depositors' aria-hidden='true' />Depositors</td>
                            <td className='lending-muted'>
                                {split.depositors_paid} {split.depositors_paid === 1 ? 'member' : 'members'}, pro-rata
                                {split.pool_balance !== null && <> to {pesos(split.pool_balance)} in balances</>}
                            </td>
                            <td>{shareOf(parts.depositors, interest)}</td>
                            <td>{pesos(parts.depositors)}</td>
                        </tr>
                        <tr>
                            <td><span className='lending-split-swatch is-reserve' aria-hidden='true' />Lending reserve</td>
                            <td className='lending-muted'>Fixed reserve share</td>
                            <td>{shareOf(parts.reserve, interest)}</td>
                            <td>{pesos(parts.reserve)}</td>
                        </tr>
                        <tr>
                            <td><span className='lending-split-swatch is-platform' aria-hidden='true' />Platform</td>
                            <td className='lending-muted'>Fixed platform share</td>
                            <td>{shareOf(parts.platform, interest)}</td>
                            <td>{pesos(parts.platform)}</td>
                        </tr>
                        {split.guarantors.map((g, i) => (
                            <tr key={i}>
                                <td>
                                    <span className='lending-split-swatch is-guarantor' aria-hidden='true' />
                                    {g.position ? `Guarantor ${g.position}` : 'Guarantor'}
                                </td>
                                <td className='lending-muted'>
                                    {g.tier_share !== null ? `${g.tier_share}% tier` : 'Guarantor share'}
                                    {split.pledged_total ? <>, by pledge of {pesos(split.pledged_total)}</> : null}
                                </td>
                                <td>{shareOf(g.amount, interest)}</td>
                                <td>{pesos(g.amount)}</td>
                            </tr>
                        ))}
                        <tr>
                            <td><span className='lending-split-swatch is-recovery_fund' aria-hidden='true' />Recovery fund</td>
                            <td className='lending-muted'>Rest of the risk band</td>
                            <td>{shareOf(parts.recovery_fund, interest)}</td>
                            <td>{pesos(parts.recovery_fund)}</td>
                        </tr>
                        <tr className='lending-split-table-total'>
                            <td>Total interest</td>
                            <td className='lending-muted'>
                                {split.policy_version !== null ? `Policy v${split.policy_version}` : ''}
                            </td>
                            <td>100%</td>
                            <td>{pesos(interest)}</td>
                        </tr>
                    </tbody>
                </table>
            </div>
        </>
    )
}

/**
 * One loan's whole public record: terms and dates, what backed it (and what
 * is still locked), the schedule, every repayment with where its interest
 * went, and — if it went bad — the recovery waterfall. Every on-chain step
 * links to the transaction that proves it.
 */
function RecordDetail({ record }: { record: PublicLoanDetail }) {
    const { loan, collateral } = record
    const lockedNow = record.locked_now.collateral + record.locked_now.lent + record.locked_now.pledged

    return (
        <>
            <section className='lending-card'>
                <div className='records-detail-head'>
                    <div>
                        <span className='lending-stat-label'>Loan #{loanRef(loan.id)}</span>
                        <b className='records-detail-amount'>{pesos(loan.principal)}</b>
                        <span className='lending-muted'>
                            {PRODUCT_LABEL[loan.product] ?? loan.product} · {rate(loan.rate_bps)} · {loan.term_months} months
                            {' '}· policy v{record.policy_version}
                        </span>
                    </div>
                    <span className={`lending-loan-status ${STATUS_CLS[loan.status]}`}>
                        {PUBLIC_STATUS_LABEL[loan.status] ?? loan.status}
                    </span>
                </div>
                <p className='records-ref-full'>{loan.id}</p>

                <div className='lending-pool-tiles'>
                    <div className='lending-funds-tile'>
                        <span className='lending-stat-label'>Outstanding</span>
                        <span className='lending-stat-value'>{pesos(loan.principal_outstanding)}</span>
                    </div>
                    <div className='lending-funds-tile'>
                        <span className='lending-stat-label'>Repaid</span>
                        <span className='lending-stat-value'>{pesos(loan.repaid)}</span>
                    </div>
                    <div className='lending-funds-tile'>
                        <span className='lending-stat-label'>Interest paid</span>
                        <span className='lending-stat-value is-good'>{pesos(loan.interest_paid)}</span>
                    </div>
                </div>

                <ol className='records-timeline'>
                    {timeline(record).map(step => (
                        <li key={step.label} className={step.tone ? `is-${step.tone}` : undefined}>
                            <b>{step.label}</b>
                            <span className='lending-muted'>{formatDate(step.at)}</span>
                        </li>
                    ))}
                </ol>
            </section>

            <section className='lending-card'>
                <div className='lending-card-head'>
                    <span className='lending-card-icon is-accent'><ShieldCheck /></span>
                    <h2>What backed it</h2>
                </div>

                <dl className='records-facts'>
                    <div>
                        <dt>Borrower’s deposit locked</dt>
                        <dd>{pesos(loan.deposit_locked)}</dd>
                    </div>
                    {loan.product === 'guarantor' && (
                        <div>
                            <dt>Borrower had to cover</dt>
                            <dd>{pesos(record.borrower_cover)}</dd>
                        </div>
                    )}
                    <div>
                        <dt>Funded from depositors’ balances</dt>
                        <dd>{record.pool_funded !== null ? pesos(record.pool_funded) : '—'}</dd>
                    </div>
                    <div>
                        <dt>Guarantor deposits locked</dt>
                        <dd>{pesos(loan.guarantor_locked)}</dd>
                    </div>
                    <div>
                        <dt>XLM locked</dt>
                        <dd>{collateral ? xlm(collateral.locked_stroops) : '—'}</dd>
                    </div>
                    <div>
                        <dt>Still locked today</dt>
                        <dd>{pesos(lockedNow)}</dd>
                    </div>
                </dl>
                {lockedNow > 0 && (
                    <p className='lending-muted'>
                        Locked today: {pesos(record.locked_now.collateral)} borrower deposit · {pesos(record.locked_now.lent)} depositors’
                        funding · {pesos(record.locked_now.pledged)} guarantor pledges
                    </p>
                )}

                {collateral && (
                    <div className='admin-movements'>
                        <span className='lending-stat-label'>XLM collateral</span>
                        <div className='admin-movement'>
                            <div className='admin-movement-what'>
                                <b>{COLLATERAL_STATUS_LABEL[collateral.status] ?? collateral.status}</b>
                                <span className='lending-muted'>
                                    {xlm(collateral.required_stroops)} required at {collateral.collateral_ratio_bps / 100}% cover
                                    {collateral.covers_centavos !== null && <> of {pesos(collateral.covers_centavos)}</>}
                                    {collateral.locked_at !== null && <> · locked {formatDate(collateral.locked_at)}</>}
                                </span>
                            </div>
                            {collateral.lock_tx_hash ? <TxLink hash={collateral.lock_tx_hash} /> : <span className='admin-movement-ref'>—</span>}
                        </div>
                        {collateral.actions.map((action, i) => (
                            <div key={i} className='admin-movement'>
                                <div className='admin-movement-what'>
                                    <b>{VAULT_ACTION_LABEL[action.action] ?? action.action}</b>
                                    <span className='lending-muted'>
                                        {formatDate(action.at)}
                                        {action.moved_stroops !== null && ` · ${xlm(action.moved_stroops)}`}
                                        {action.value_centavos !== null && ` · worth ${pesos(action.value_centavos)}`}
                                        {action.status === 'queued' && ' · waiting to be signed'}
                                    </span>
                                </div>
                                {action.tx_hash ? <TxLink hash={action.tx_hash} /> : <span className='admin-movement-ref'>—</span>}
                            </div>
                        ))}
                    </div>
                )}

                {record.guarantors.length > 0 && (
                    <div className='lending-rates-scroll'>
                        <table className='lending-rates-table'>
                            <thead>
                                <tr><th>Guarantor</th><th>Pledge</th><th>Status</th></tr>
                            </thead>
                            <tbody>
                                {record.guarantors.map(g => (
                                    <tr key={g.position}>
                                        <td>Guarantor {g.position}</td>
                                        <td>{pesos(g.pledge_amount)}</td>
                                        <td>{PLEDGE_STATUS_LABEL[g.status] ?? g.status}</td>
                                    </tr>
                                ))}
                            </tbody>
                        </table>
                    </div>
                )}
            </section>

            {record.schedule.length > 0 && (
                <section className='lending-card'>
                    <div className='lending-card-head'>
                        <span className='lending-card-icon is-accent'><Landmark /></span>
                        <h2>Schedule</h2>
                    </div>
                    <div className='lending-schedule-scroll'>
                        <table className='lending-schedule'>
                            <thead>
                                <tr><th>#</th><th>Due</th><th>Principal</th><th>Interest</th><th>Paid</th><th>Status</th></tr>
                            </thead>
                            <tbody>
                                {record.schedule.map(row => (
                                    <tr key={row.installment} className={row.status === 'paid' ? 'is-paid' : undefined}>
                                        <td>{row.installment}</td>
                                        <td>{formatDate(row.due_at)}</td>
                                        <td>{pesos(row.principal_due)}</td>
                                        <td>{pesos(row.interest_due)}</td>
                                        <td>{pesos(row.principal_paid + row.interest_paid)}</td>
                                        <td>{INSTALLMENT_STATUS_LABEL[row.status] ?? row.status}</td>
                                    </tr>
                                ))}
                            </tbody>
                        </table>
                    </div>
                </section>
            )}

            <section className='lending-card lending-card-split'>
                <div className='lending-card-head'>
                    <span className='lending-card-icon is-accent'><Split /></span>
                    <h2>Repayments and interest split</h2>
                </div>
                {record.payments.length === 0 ? (
                    <p className='lending-muted'>No repayments on this loan.</p>
                ) : (
                    record.payments.map((payment, i) => (
                        <article key={i} className='records-payment'>
                            <div className='records-payment-head'>
                                <Receipt aria-hidden='true' />
                                <div>
                                    <b>{pesos(payment.amount)}</b>
                                    <span className='lending-muted'>
                                        {formatDate(payment.paid_at)} · {pesos(payment.principal_paid)} principal ·{' '}
                                        {pesos(payment.interest_paid)} interest
                                        {payment.fee_paid > 0 && <> · {pesos(payment.fee_paid)} provider fee paid on top</>}
                                    </span>
                                </div>
                            </div>
                            <SplitTable payment={payment} />
                        </article>
                    ))
                )}
            </section>

            {record.recoveries.length > 0 && (
                <section className='lending-card'>
                    <div className='lending-card-head'>
                        <span className='lending-card-icon is-accent'><AlertTriangle /></span>
                        <h2>Default recovery</h2>
                    </div>
                    <p className='lending-muted'>
                        Taken in order: the borrower’s own deposit, their XLM, the guarantors’ pledges, then the
                        recovery fund and the lending reserve.
                    </p>
                    <div className='lending-rates-scroll'>
                        <table className='lending-rates-table'>
                            <thead>
                                <tr><th>Step</th><th>Taken from</th><th>Amount</th><th>Given back</th><th>Date</th></tr>
                            </thead>
                            <tbody>
                                {record.recoveries.map((step, i) => (
                                    <tr key={i}>
                                        <td>{step.step}</td>
                                        <td>
                                            {recoverySource(step)}
                                            {step.stroops !== null && <span className='lending-muted'> · {xlm(step.stroops)}</span>}
                                        </td>
                                        <td>{pesos(step.amount)}</td>
                                        <td>{step.refunded > 0 ? pesos(step.refunded) : '—'}</td>
                                        <td>{formatDate(step.at)}</td>
                                    </tr>
                                ))}
                            </tbody>
                        </table>
                    </div>
                </section>
            )}
        </>
    )
}

export default RecordDetail
