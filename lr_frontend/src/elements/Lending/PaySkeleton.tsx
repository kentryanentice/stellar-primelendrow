import { SkeletonBone, PaymentRowsSkeleton } from './Skeleton'

/**
 * RepayCard's body only — tile grid through the schedule table, no card-head
 * or `<section>` wrapper. RepayCard.tsx renders its own head unconditionally
 * (above its `loading` branch), so its in-place loading state uses this
 * directly instead of the section-wrapped RepayCardSkeleton below, which
 * would duplicate the head.
 *
 * Modeled on RepayCard's "has an active loan with a payment due" branch,
 * not its loading/empty/no-loan states — those are structurally a
 * centered icon+message, nothing like the tile grid + schedule table this
 * page exists to show, and a caller opening /pay to actually pay something
 * is the case this page is for. A caller with nothing due sees the loaded
 * empty state differ from this skeleton's shape, same limit as Borrow's
 * conditional open-loan card.
 */
export function RepayCardBody() {
    return (
        <>
            <div className='lending-pay-loan'>
                <div className='lending-loan-title'>
                    <b><SkeletonBone width={90} height={15} /></b>
                    <span><SkeletonBone width={160} height={12} /></span>
                </div>
                <span className='lending-loan-status'><SkeletonBone width={54} height={18} radius={999} /></span>
            </div>

            <div className='lending-funds-grid'>
                {[0, 1, 2, 3].map(i => (
                    <div key={i} className='lending-funds-tile'>
                        <span className='lending-stat-label'><SkeletonBone width={64} height={11} /></span>
                        <span className='lending-stat-value'><SkeletonBone width={78} height={19} /></span>
                    </div>
                ))}
            </div>

            <p className='lending-muted'><SkeletonBone width={150} height={13} /></p>

            <span className='lending-label'><SkeletonBone width={110} height={12} /></span>
            <div className='lending-payment-method' aria-hidden='true'>
                <span className='lending-payment-method-icon' />
                <span className='lending-payment-method-info'>
                    <b><SkeletonBone width={50} height={13} /></b>
                    <span><SkeletonBone width={140} height={12} /></span>
                </span>
            </div>
            <SkeletonBone width='100%' height={42} radius={10} />

            <span className='lending-stat-label'><SkeletonBone width={130} height={11} /></span>
            <div className='lending-schedule-scroll'>
                <table className='lending-schedule' aria-hidden='true'>
                    <thead><tr><th>#</th><th>Due</th><th>Principal</th><th>Interest</th><th>Status</th></tr></thead>
                    <tbody>
                        {[0, 1, 2].map(i => (
                            <tr key={i}>
                                <td><SkeletonBone width={12} height={12} /></td>
                                <td><SkeletonBone width={70} height={12} /></td>
                                <td><SkeletonBone width={60} height={12} /></td>
                                <td><SkeletonBone width={50} height={12} /></td>
                                <td><SkeletonBone width={56} height={12} /></td>
                            </tr>
                        ))}
                    </tbody>
                </table>
            </div>
        </>
    )
}

/**
 * RepayCard's full section, head included — Pay.tsx's Suspense fallback for
 * it (it's lazy-loaded to keep the PayPal SDK bootstrap out of the initial
 * bundle).
 */
export function RepayCardSkeleton() {
    return (
        <section className='lending-card lending-card-repay'>
            <div className='lending-card-head'>
                <span className='lending-card-icon is-accent' aria-hidden='true' />
                <h2><SkeletonBone width={140} height={16} /></h2>
            </div>
            <RepayCardBody />
        </section>
    )
}

/**
 * PaymentHistoryCard's full section — Pay.tsx's Suspense fallback for it,
 * same rationale as RepayCardSkeleton above.
 */
export function PaymentHistoryCardSkeleton() {
    return (
        <section className='lending-card lending-card-payment-history'>
            <div className='lending-card-head'>
                <span className='lending-card-icon is-accent' aria-hidden='true' />
                <h2><SkeletonBone width={120} height={16} /></h2>
            </div>
            <ul className='lending-payment-history' aria-hidden='true'>
                <PaymentRowsSkeleton />
            </ul>
        </section>
    )
}

/**
 * Mirrors Pay.tsx's loaded shape — still the older .lending-columns /
 * .lending-column two-up grid (never migrated to the rail-layout pattern
 * the other two pages use), so that's what this reuses. See Skeleton.tsx
 * for why this is hand-composed rather than one generic wrapper.
 *
 * Doesn't include the page header: unlike Lending/Borrow, Pay.tsx's header
 * carries no live data (no dynamic pill or note), so it's already rendered
 * unconditionally above this component — duplicating it here would show it
 * twice while loading.
 */
function PaySkeleton() {
    return (
        <div className='lending-columns'>
            <div className='lending-column'>
                <RepayCardSkeleton />
            </div>

            <div className='lending-column'>
                <section className='lending-card lending-card-payment-summary'>
                    <div className='lending-card-head'>
                        <span className='lending-card-icon is-accent' aria-hidden='true' />
                        <h2><SkeletonBone width={110} height={16} /></h2>
                    </div>
                    <div className='lending-eligibility-score'>
                        <span className='lending-eligibility-score-value'><SkeletonBone width={110} height={34} /></span>
                        <span className='lending-eligibility-score-max'><SkeletonBone width={80} height={13} /></span>
                    </div>
                    <div className='lending-eligibility-rows'>
                        {[0, 1].map(i => (
                            <div key={i} className='lending-quote-row'>
                                <span><SkeletonBone width={100} height={13} /></span>
                                <b><SkeletonBone width={70} height={13} /></b>
                            </div>
                        ))}
                    </div>
                </section>

                <PaymentHistoryCardSkeleton />
            </div>
        </div>
    )
}

export default PaySkeleton
