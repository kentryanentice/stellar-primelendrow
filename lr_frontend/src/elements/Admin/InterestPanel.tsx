import { useState } from 'react'
import { ChevronLeft, ChevronRight, Search } from 'lucide-react'
import type useAdminInterest from '../../functions/Admin/useAdminInterest'
import type { InterestPayment, InterestRecipient } from '../../functions/Admin/useAdminInterest'
import { formatDate, pesos } from '../../functions/Lending/money'
import { shortId } from '../../functions/Lending/explorer'
import { RECIPIENTS, widthPct } from '../../functions/Lending/split'
import { PRODUCT_LABEL } from '../../functions/Lending/types'

/** A share of a whole, as a percent label ("12.5%"). Display only — the
 *  amounts beside it are the engine's; this only says what fraction they are. */
const shareOf = (part: number, whole: number) =>
    whole > 0 ? `${((part / whole) * 100).toFixed(2).replace(/\.?0+$/, '')}%` : '—'

/** Why a member received their slice, in the operator's words. */
function reasonFor(r: InterestRecipient, payment: InterestPayment) {
    if (r.role === 'guarantor') {
        const pledged = payment.pledged_total
        return `Guarantor at the ${r.tier_share ?? 0}% tier · pledged ${pesos(r.weight)}`
            + (pledged ? ` of ${pesos(pledged)} (${shareOf(r.weight, pledged)} of the pledges)` : '')
    }
    const pool = payment.pool_balance
    return pool === null
        // Recorded before the pool total was captured (040): the weight is
        // still the basis, there is just no total to set it against.
        ? `Depositor · weighted by ${pesos(r.weight)}`
        : `Depositor · ${pesos(r.weight)} of ${pesos(pool)} in the pool (${shareOf(r.weight, pool)})`
}

/** The parts of a payment that go to a fund rather than a member. */
const FUND_ROWS: { key: 'platform' | 'reserve' | 'recovery_fund'; label: string; reason: string }[] = [
    { key: 'platform', label: 'Platform fee', reason: 'Fixed platform share' },
    { key: 'reserve', label: 'Lending reserve', reason: 'Fixed reserve share' },
    { key: 'recovery_fund', label: 'Recovery fund', reason: 'Rest of the risk band after guarantors' },
]

/**
 * Where every repayment's interest went (POST /lending/admin/interest).
 *
 * One card per payment, newest first: the split as a strip in the same colors
 * the member-facing card uses, then every line of it — the three funds, each
 * guarantor, each depositor — with who got it, why, and how much. The lines
 * always add back to the payment's interest, so the card is its own check.
 * Read-only: nothing here moves money.
 */
function InterestPanel({ interest }: { interest: ReturnType<typeof useAdminInterest> }) {
    const { payments, page, total, totalPages, search, setSearch, loading, error, goToPage } = interest
    const [draft, setDraft] = useState(search)

    return (
        <section className='admin-loans'>
            <div className='admin-loans-head'>
                <h2>Interest history</h2>
                <form
                    className='admin-interest-search'
                    onSubmit={e => {
                        e.preventDefault()
                        setSearch(draft.trim())
                    }}
                >
                    <input
                        className='lending-input'
                        type='search'
                        placeholder='Borrower or member username'
                        value={draft}
                        maxLength={64}
                        onChange={e => setDraft(e.target.value)}
                    />
                    <button type='submit' className='lending-btn' aria-label='Search'><Search /></button>
                </form>
            </div>

            {loading ? (
                <p className='lending-muted'>Loading interest history…</p>
            ) : error ? (
                <p className='lending-muted'>Couldn’t load the interest history. Try refreshing.</p>
            ) : total === 0 ? (
                <p className='lending-muted'>
                    {search ? `No repayments involving “${search}”.` : 'No interest has been collected yet.'}
                </p>
            ) : (
                <>
                    <p className='lending-muted'>{total} {total === 1 ? 'repayment' : 'repayments'}</p>
                    {payments.map(payment => {
                        const guarantors = payment.recipients.filter(r => r.role === 'guarantor')
                        const depositors = payment.recipients.filter(r => r.role === 'depositor')
                        const drawn = RECIPIENTS.filter(r => payment.parts[r.key] > 0)
                        return (
                            <article key={payment.event_id} className='lending-card lending-card-split admin-interest-payment'>
                                <div className='admin-interest-head'>
                                    <div>
                                        <b>{pesos(payment.interest)} interest</b>
                                        <span className='lending-muted'>
                                            {payment.borrower} · {PRODUCT_LABEL[payment.product] ?? payment.product}
                                            {' · '}loan {shortId(payment.loan_id)} · {formatDate(payment.paid_at)}
                                        </span>
                                    </div>
                                </div>

                                <div className='lending-split-bar is-thin' aria-hidden='true'>
                                    {drawn.map(r => (
                                        <span
                                            key={r.key}
                                            className={`lending-split-seg is-${r.key}`}
                                            style={{ width: `${widthPct(payment.parts[r.key], payment.interest)}%` }}
                                        />
                                    ))}
                                </div>

                                <div className='lending-rates-scroll'>
                                    <table className='lending-rates-table admin-interest-table'>
                                        <thead>
                                            <tr><th>Recipient</th><th>Why</th><th>Share of interest</th><th>Amount</th></tr>
                                        </thead>
                                        <tbody>
                                            {FUND_ROWS.filter(f => payment.parts[f.key] > 0).map(f => (
                                                <tr key={f.key}>
                                                    <td><span className={`lending-split-swatch is-${f.key}`} aria-hidden='true' />{f.label}</td>
                                                    <td className='lending-muted'>{f.reason}</td>
                                                    <td>{shareOf(payment.parts[f.key], payment.interest)}</td>
                                                    <td>{pesos(payment.parts[f.key])}</td>
                                                </tr>
                                            ))}
                                            {guarantors.map(r => (
                                                <tr key={`g-${r.username}`}>
                                                    <td><span className='lending-split-swatch is-guarantor' aria-hidden='true' />{r.username}</td>
                                                    <td className='lending-muted'>{reasonFor(r, payment)}</td>
                                                    <td>{shareOf(r.amount, payment.interest)}</td>
                                                    <td>{pesos(r.amount)}</td>
                                                </tr>
                                            ))}
                                            {depositors.map(r => (
                                                <tr key={`d-${r.username}`}>
                                                    <td><span className='lending-split-swatch is-depositors' aria-hidden='true' />{r.username}</td>
                                                    <td className='lending-muted'>{reasonFor(r, payment)}</td>
                                                    <td>{shareOf(r.amount, payment.interest)}</td>
                                                    <td>{pesos(r.amount)}</td>
                                                </tr>
                                            ))}
                                            <tr className='lending-split-table-total'>
                                                <td>Total</td>
                                                <td className='lending-muted'>
                                                    Depositors {pesos(payment.parts.depositors)} ({shareOf(payment.parts.depositors, payment.interest)})
                                                    {' '}across {depositors.length}
                                                    {depositors.length === 1 ? ' member' : ' members'}
                                                    {payment.parts.guarantor > 0
                                                        && ` · guarantors ${pesos(payment.parts.guarantor)} (${shareOf(payment.parts.guarantor, payment.interest)})`}
                                                </td>
                                                <td>100%</td>
                                                <td>{pesos(payment.interest)}</td>
                                            </tr>
                                        </tbody>
                                    </table>
                                </div>
                            </article>
                        )
                    })}

                    {totalPages > 1 && (
                        <div className='lending-pager'>
                            <button
                                type='button'
                                className='lending-pager-btn'
                                aria-label='Previous page'
                                disabled={page <= 1}
                                onClick={() => goToPage(page - 1)}
                            >
                                <ChevronLeft />
                            </button>
                            <span className='lending-muted'>Page {page} of {totalPages}</span>
                            <button
                                type='button'
                                className='lending-pager-btn'
                                aria-label='Next page'
                                disabled={page >= totalPages}
                                onClick={() => goToPage(page + 1)}
                            >
                                <ChevronRight />
                            </button>
                        </div>
                    )}
                </>
            )}
        </section>
    )
}

export default InterestPanel
