import { SkeletonBone } from '../Lending/Skeleton'

const TILE_VALUE_WIDTHS = [124, 112, 104, 96]
const ACTIVITY_DETAIL_WIDTHS = [150, 220, 170, 250, 140, 200]

/**
 * Activity rows while their three sources load. Exported so
 * RecentActivityCard's own loading branch and the full-page skeleton below
 * share one shape (same reasoning as the row fragments in Lending/Skeleton).
 */
export function ActivityRowsSkeleton({ rows = 6 }: { rows?: number }) {
    return (
        <ul className='dash-activity-list' aria-hidden='true'>
            {Array.from({ length: rows }).map((_, i) => (
                <li key={i} className='dash-activity-row'>
                    <span className='dash-activity-icon' />
                    <span className='dash-activity-what'>
                        <SkeletonBone width={80} height={14} />
                        <SkeletonBone width={ACTIVITY_DETAIL_WIDTHS[i % ACTIVITY_DETAIL_WIDTHS.length]} height={11} />
                    </span>
                    <span className='dash-activity-status'><SkeletonBone width={70} height={18} radius={999} /></span>
                    <span className='dash-activity-date'><SkeletonBone width={72} height={12} /></span>
                </li>
            ))}
        </ul>
    )
}

/** The next-payment figure and its actions, before GET /loans resolves. */
export function NextPaymentBodySkeleton() {
    return (
        <div className='dash-due-row' aria-hidden='true'>
            <div className='dash-due-figure'>
                <SkeletonBone width={150} height={34} />
                <SkeletonBone width={280} height={13} />
            </div>
            <div className='dash-due-actions'>
                <SkeletonBone width={124} height={38} radius={10} />
                <SkeletonBone width={124} height={38} radius={10} />
            </div>
        </div>
    )
}

/**
 * Mirrors Dashboard.tsx's loaded shape using the same wrapper classes, so the
 * grid rules that size the page are shared with the loaded state rather than
 * re-approximated here.
 */
function DashboardSkeleton() {
    return (
        <>
            <header className='dash-head' aria-hidden='true'>
                <div>
                    <p className='dash-eyebrow'><SkeletonBone width={64} height={11} /></p>
                    <h1><SkeletonBone width={250} height={30} /></h1>
                    <p><SkeletonBone width={300} height={13} /></p>
                </div>
                <div className='dash-head-meta'>
                    <SkeletonBone width={170} height={34} radius={10} />
                </div>
            </header>

            <div className='dash-tiles' aria-hidden='true'>
                {TILE_VALUE_WIDTHS.map((width, i) => (
                    <div key={i} className='dash-tile'>
                        <SkeletonBone width={90} height={11} />
                        <SkeletonBone width={width} height={26} />
                        <SkeletonBone width={140} height={12} />
                    </div>
                ))}
            </div>

            <div className='dash-layout' aria-hidden='true'>
                <div className='dash-rail'>
                    <section className='lending-card'>
                        <SkeletonBone width={90} height={11} />
                        <SkeletonBone width={170} height={34} />
                        <div className='dash-rows'>
                            {[0, 1].map(i => (
                                <div key={i}>
                                    <SkeletonBone width={110} height={13} />
                                    <SkeletonBone width={80} height={13} />
                                </div>
                            ))}
                        </div>
                        <div className='dash-actions'>
                            <SkeletonBone width='100%' height={38} radius={10} />
                            <SkeletonBone width='100%' height={38} radius={10} />
                        </div>
                    </section>
                    <section className='lending-card'>
                        <SkeletonBone width={90} height={11} />
                        <SkeletonBone width={110} height={36} />
                        <SkeletonBone width='100%' height={9} radius={999} />
                        <SkeletonBone width='85%' height={13} />
                    </section>
                    <section className='lending-card'>
                        <SkeletonBone width={110} height={11} />
                        <div className='dash-limits'>
                            {[0, 1, 2].map(i => (
                                <div key={i}>
                                    <SkeletonBone width={60} height={11} />
                                    <SkeletonBone width={64} height={16} />
                                </div>
                            ))}
                        </div>
                    </section>
                </div>

                <div className='dash-main'>
                    <section className='lending-card'>
                        <div className='lending-card-head'>
                            <span className='lending-card-icon is-accent' />
                            <h2><SkeletonBone width={150} height={16} /></h2>
                        </div>
                        <NextPaymentBodySkeleton />
                        <SkeletonBone width='100%' height={7} radius={999} />
                    </section>
                    <section className='lending-card'>
                        <div className='lending-card-head'>
                            <span className='lending-card-icon is-accent' />
                            <h2><SkeletonBone width={130} height={16} /></h2>
                        </div>
                        <ActivityRowsSkeleton />
                    </section>
                </div>
            </div>
        </>
    )
}

export default DashboardSkeleton
