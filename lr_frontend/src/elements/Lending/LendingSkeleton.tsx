import { SkeletonBone, LedgerRowsSkeleton, PagerSkeleton } from './Skeleton'

const TILE_LABEL_WIDTHS = [62, 84, 96, 88]
const TILE_VALUE_WIDTHS = [78, 66, 72, 48]

/**
 * The funds rail — same shape ManageFundsCard.tsx loads into. Exported on
 * its own so Lending.tsx can also use it as that card's Suspense fallback
 * (it's lazy-loaded to keep the PayPal SDK bootstrap out of the initial
 * bundle), not just as part of the full-page skeleton below.
 */
export function ManageFundsCardSkeleton() {
    return (
        <section className='lending-card lending-card-funds'>
            <div className='lending-rail-balance'>
                <span className='lending-stat-label'><SkeletonBone width={90} height={11} /></span>
                <div className='lending-rail-balance-row'>
                    <span className='lending-rail-balance-value'><SkeletonBone width={140} height={32} /></span>
                </div>
            </div>
            <div className='lending-rail-secondary'>
                {[0, 1].map(i => (
                    <div key={i}>
                        <SkeletonBone width={100} height={13} />
                        <SkeletonBone width={70} height={13} />
                    </div>
                ))}
            </div>
            <p className='lending-muted lending-locked-breakdown' aria-hidden='true'>
                <SkeletonBone width={13} height={13} radius={3} />
                <SkeletonBone width={200} height={13} />
            </p>
            <div className='lending-tab-group' aria-hidden='true'>
                <SkeletonBone width='100%' height={43} radius={7} />
            </div>
            <div className='lending-rail-form'>
                <div className='lending-rail-input'>
                    <SkeletonBone width='100%' height={52} radius={8} />
                </div>
                <p className='lending-muted' style={{ display: 'flex', flexDirection: 'column', gap: 5 }}>
                    <SkeletonBone width='95%' height={13} />
                    <SkeletonBone width='55%' height={13} />
                </p>
                <SkeletonBone width='100%' height={64} radius={10} />
            </div>
        </section>
    )
}

/**
 * Mirrors Lending.tsx's loaded shape exactly — same wrapper classes
 * (.lending-pool-tiles, .lending-rail-layout, .lending-card-funds, the
 * ledger table), so the grid/flex/padding rules that actually determine the
 * page's dimensions are shared with the loaded state, not re-approximated
 * here. See Skeleton.tsx for why this is hand-composed rather than one
 * generic wrapper.
 */
function LendingSkeleton() {
    return (
        <>
            <header className='lending-head lending-head-with-pill'>
                <div>
                    <p className='lending-eyebrow'><SkeletonBone width={58} height={11} /></p>
                    <h1><SkeletonBone width={210} height={28} /></h1>
                </div>
            </header>

            <div className='lending-pool-tiles'>
                {TILE_LABEL_WIDTHS.map((labelWidth, i) => (
                    <div key={i} className='lending-funds-tile'>
                        <span className='lending-stat-label'><SkeletonBone width={labelWidth} height={11} /></span>
                        <span className='lending-stat-value'><SkeletonBone width={TILE_VALUE_WIDTHS[i]} height={19} /></span>
                    </div>
                ))}
            </div>

            <section className='lending-card lending-card-rates'>
                <div className='lending-tier-summary'>
                    <div className='lending-tier-summary-score'>
                        <span className='lending-stat-label'><SkeletonBone width={64} height={11} /></span>
                        <span className='lending-tier-summary-band'><SkeletonBone width={90} height={15} /></span>
                    </div>
                    <div className='lending-tier-summary-divider' />
                    <div className='lending-tier-summary-stats'>
                        {[54, 46, 60].map((w, i) => (
                            <div key={i}>
                                <span className='lending-stat-label'><SkeletonBone width={w} height={11} /></span>
                                <span className='lending-stat-value'><SkeletonBone width={w + 10} height={15} /></span>
                            </div>
                        ))}
                    </div>
                    <span className='lending-btn lending-tier-toggle' aria-hidden='true'><SkeletonBone width={90} height={13} /></span>
                </div>
            </section>

            <div className='lending-rail-layout'>
                <aside className='lending-rail'>
                    <ManageFundsCardSkeleton />
                </aside>

                <div className='lending-main'>
                    <section className='lending-card lending-card-deposits'>
                        <div className='lending-card-head'>
                            <span className='lending-card-icon is-accent' aria-hidden='true' />
                            <h2><SkeletonBone width={110} height={16} /></h2>
                        </div>
                        <div className='lending-rates-scroll'>
                            <table className='lending-ledger-table' aria-hidden='true'>
                                <thead>
                                    <tr><th>Amount</th><th>Status</th><th>Deposited</th></tr>
                                </thead>
                                <tbody>
                                    <LedgerRowsSkeleton />
                                </tbody>
                            </table>
                        </div>
                        <PagerSkeleton />
                    </section>

                    <div className='lending-guarantor-empty' aria-hidden='true'>
                        <span className='lending-guarantor-empty-icon' />
                        <SkeletonBone width='70%' height={13} />
                    </div>
                </div>
            </div>
        </>
    )
}

export default LendingSkeleton
