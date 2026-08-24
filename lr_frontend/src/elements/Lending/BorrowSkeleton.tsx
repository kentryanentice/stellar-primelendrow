import { SkeletonBone, LoanRowsSkeleton, OpenLoanSkeleton } from './Skeleton'

/**
 * The credit-score card — same shape EligibilityCard.tsx loads into. Not
 * lazy/Suspense-wrapped in Borrow.tsx, so unlike the two skeletons below it
 * this one's only used inline here, but it's still split out to keep this
 * file's default export a plain composition of named pieces.
 */
function EligibilityCardSkeleton() {
    return (
        <section className='lending-card lending-card-eligibility'>
            <div className='lending-eligibility-head'>
                <div>
                    <span className='lending-stat-label'><SkeletonBone width={78} height={11} /></span>
                    <div className='lending-eligibility-score'>
                        <span className='lending-eligibility-score-value'><SkeletonBone width={54} height={34} /></span>
                        <span className='lending-eligibility-score-max'><SkeletonBone width={40} height={13} /></span>
                    </div>
                </div>
                <span className='lending-pill is-accent' aria-hidden='true'><SkeletonBone width={110} height={13} /></span>
            </div>
            <div className='lending-utilization-track'><div className='lending-utilization-fill' style={{ width: '40%' }} /></div>
            <p className='lending-muted' style={{ display: 'flex', flexDirection: 'column', gap: 7 }}>
                <SkeletonBone width='95%' height={16} />
                <SkeletonBone width='55%' height={16} />
            </p>
        </section>
    )
}

/**
 * The apply-for-a-loan card — same shape BorrowCard.tsx loads into. Exported
 * on its own so Borrow.tsx can also use it as that card's Suspense fallback
 * (it's lazy-loaded to keep the wallet kit out of the initial bundle), not
 * just as part of the full-page skeleton below.
 */
export function BorrowCardSkeleton() {
    return (
        <section className='lending-card lending-card-borrow'>
            <div className='lending-card-head'>
                <span className='lending-card-icon is-accent' aria-hidden='true' />
                <h2><SkeletonBone width={190} height={16} /></h2>
            </div>
            <p className='lending-muted'><SkeletonBone width='75%' height={13} /></p>

            <div className='lending-product-tiles'>
                {[0, 1, 2].map(i => (
                    <div key={i} className='lending-product-tile' aria-hidden='true'>
                        <span className='lending-product-tile-label'><SkeletonBone width={100} height={13} /></span>
                        <span className='lending-product-tile-rate'><SkeletonBone width={54} height={19} /></span>
                        <span className='lending-muted'><SkeletonBone width={90} height={12} /></span>
                    </div>
                ))}
            </div>

            <p className='lending-muted'><SkeletonBone width='80%' height={13} /></p>

            <div className='lending-borrow-split'>
                <div className='lending-borrow-fields'>
                    <div className='lending-borrow-form'>
                        <div className='lending-field'>
                            <span className='lending-label'><SkeletonBone width={80} height={12} /></span>
                            <div className='lending-input' aria-hidden='true'><SkeletonBone width={70} height={16} /></div>
                        </div>
                        <div className='lending-field'>
                            <span className='lending-label'><SkeletonBone width={40} height={12} /></span>
                            <div className='lending-input' aria-hidden='true'><SkeletonBone width={80} height={16} /></div>
                        </div>
                    </div>
                    <label className='lending-consent' aria-hidden='true'>
                        <SkeletonBone width={15} height={15} radius={4} />
                        <SkeletonBone width='95%' height={30} />
                    </label>
                    <span className='lending-btn-primary' aria-hidden='true'><SkeletonBone width={140} height={15} /></span>
                </div>
                <div className='lending-borrow-quotepanel'>
                    <span className='lending-stat-label'><SkeletonBone width={70} height={11} /></span>
                    <p className='lending-muted'><SkeletonBone width='85%' height={13} /></p>
                </div>
            </div>
        </section>
    )
}

/**
 * The loan-history card — same shape LoanHistoryCard.tsx loads into.
 * Exported for the same reason as BorrowCardSkeleton above: it doubles as
 * that card's own Suspense fallback in Borrow.tsx.
 */
export function LoanHistoryCardSkeleton() {
    return (
        <section className='lending-card lending-card-loans'>
            <div className='lending-card-head'>
                <span className='lending-card-icon is-accent' aria-hidden='true' />
                <h2><SkeletonBone width={90} height={16} /></h2>
            </div>
            <ul className='lending-loans' aria-hidden='true'>
                <LoanRowsSkeleton />
            </ul>
        </section>
    )
}

/**
 * Mirrors Borrow.tsx's loaded shape — same wrapper classes as the loaded
 * page (.lending-borrow-layout, .lending-card-eligibility, .lending-product-
 * tiles, .lending-borrow-split), so dimensions match by construction. See
 * Skeleton.tsx for why this is hand-composed rather than one generic
 * wrapper.
 *
 * Includes an OpenLoanCard-shaped block (modeled on its active-loan shape —
 * see OpenLoanSkeleton in Skeleton.tsx) even though this skeleton can't know
 * yet whether the caller actually has one open. Deliberately still NOT
 * included: the lifetime-stats block, which needs the *full* loan-history
 * array (not just "is there an open loan") to know its own three pill
 * values even exist — a guess there would be a guess about content, not
 * just shape.
 */
function BorrowSkeleton() {
    return (
        <>
            <header className='lending-head lending-head-with-pill'>
                <div>
                    <p className='lending-eyebrow'><SkeletonBone width={78} height={11} /></p>
                    <h1><SkeletonBone width={230} height={28} /></h1>
                </div>
            </header>

            <div className='lending-borrow-layout'>
                <aside className='lending-borrow-sidebar'>
                    <EligibilityCardSkeleton />
                    <OpenLoanSkeleton />
                </aside>

                <div className='lending-borrow-main'>
                    <BorrowCardSkeleton />
                    <LoanHistoryCardSkeleton />
                </div>
            </div>
        </>
    )
}

export default BorrowSkeleton
