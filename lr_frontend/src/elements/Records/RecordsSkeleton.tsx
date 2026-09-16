import { SkeletonBone } from '../Lending/Skeleton'

/**
 * Loading states for the public loan book, composed inside the same layout
 * classes the loaded page uses (tiles, .lending-loan rows, .records-facts,
 * the split table) so each piece opens at its final size and the content
 * fills in — same approach as the Lending/Borrow/Pay skeletons.
 */

/* A loan row's three lines vary in length (product names, one to three
   backing legs), so the bones vary with them. */
const PRODUCT_LINE_WIDTHS = [236, 212, 248, 198, 228, 220, 242, 206]
const BACKING_LINE_WIDTHS = [150, 214, 176, 128, 232, 160, 196, 142]

/** The book's rows — used on first load and again on every page turn or filter change. */
export function RecordRowsSkeleton({ rows = 8 }: { rows?: number }) {
    return (
        <ul className='lending-loans' role='status' aria-label='Loading loans'>
            {Array.from({ length: rows }).map((_, i) => (
                <li key={i} className='lending-loan'>
                    <div className='lending-loan-summary'>
                        <div className='lending-loan-title'>
                            <b><SkeletonBone width={96} height={15} /><span className='records-ref'><SkeletonBone width={64} height={11} /></span></b>
                            <span><SkeletonBone width={PRODUCT_LINE_WIDTHS[i % PRODUCT_LINE_WIDTHS.length]} height={12} /></span>
                            <span><SkeletonBone width={BACKING_LINE_WIDTHS[i % BACKING_LINE_WIDTHS.length]} height={12} /></span>
                        </div>
                        <span className='lending-loan-status'><SkeletonBone width={58} height={18} radius={999} /></span>
                    </div>
                </li>
            ))}
        </ul>
    )
}

/** The four totals above the book, before the first response. */
export function RecordTilesSkeleton() {
    return (
        <div className='lending-pool-tiles' aria-hidden='true'>
            {[88, 64, 52, 104].map((labelWidth, i) => (
                <div key={i} className='lending-funds-tile'>
                    <span className='lending-stat-label'><SkeletonBone width={labelWidth} height={11} /></span>
                    <span className='lending-stat-value'><SkeletonBone width={86} height={19} /></span>
                </div>
            ))}
        </div>
    )
}

function CardHeadSkeleton({ width }: { width: number }) {
    return (
        <div className='lending-card-head'>
            <span className='lending-card-icon is-accent' />
            <h2><SkeletonBone width={width} height={15} /></h2>
        </div>
    )
}

/**
 * One loan's record while it loads. Modeled on a disbursed loan with a
 * repayment — the head, what backed it, and one payment's split — rather than
 * a declined application's much shorter page: it's the shape most worth
 * opening, and the sections a smaller record lacks simply don't appear.
 */
export function RecordDetailSkeleton() {
    return (
        <div className='records-skeleton' role='status' aria-label='Loading the loan record'>
            <section className='lending-card'>
                <div className='records-detail-head'>
                    <div>
                        <span className='lending-stat-label'><SkeletonBone width={96} height={11} /></span>
                        <b className='records-detail-amount'><SkeletonBone width={140} height={26} /></b>
                        <span className='lending-muted'><SkeletonBone width={260} height={13} /></span>
                    </div>
                    <span className='lending-loan-status'><SkeletonBone width={62} height={18} radius={999} /></span>
                </div>
                <p className='records-ref-full'><SkeletonBone width={250} height={11} /></p>

                <div className='lending-pool-tiles'>
                    {[76, 52, 84].map((labelWidth, i) => (
                        <div key={i} className='lending-funds-tile'>
                            <span className='lending-stat-label'><SkeletonBone width={labelWidth} height={11} /></span>
                            <span className='lending-stat-value'><SkeletonBone width={96} height={19} /></span>
                        </div>
                    ))}
                </div>

                <ol className='records-timeline'>
                    {[52, 64, 88].map((width, i) => (
                        <li key={i}>
                            <b><SkeletonBone width={width} height={12} /></b>
                            <span className='lending-muted'><SkeletonBone width={78} height={11} /></span>
                        </li>
                    ))}
                </ol>
            </section>

            <section className='lending-card'>
                <CardHeadSkeleton width={116} />
                <dl className='records-facts'>
                    {[150, 176, 158, 112, 118].map((termWidth, i) => (
                        <div key={i}>
                            <dt><SkeletonBone width={termWidth} height={11} /></dt>
                            <dd><SkeletonBone width={92} height={15} /></dd>
                        </div>
                    ))}
                </dl>
            </section>

            <section className='lending-card'>
                <CardHeadSkeleton width={190} />
                <article className='records-payment'>
                    <div className='records-payment-head'>
                        <span className='records-skeleton-icon' />
                        <div>
                            <b><SkeletonBone width={96} height={14} /></b>
                            <span className='lending-muted'><SkeletonBone width={280} height={12} /></span>
                        </div>
                    </div>
                    <SkeletonBone width='100%' height={8} radius={4} />
                    <div className='lending-rates-scroll'>
                        <table className='lending-rates-table' aria-hidden='true'>
                            <thead><tr><th>Went to</th><th>Why</th><th>Share</th><th>Amount</th></tr></thead>
                            <tbody>
                                {[82, 104, 64, 88, 96].map((width, i) => (
                                    <tr key={i}>
                                        <td><SkeletonBone width={width} height={12} /></td>
                                        <td><SkeletonBone width={width + 40} height={12} /></td>
                                        <td><SkeletonBone width={32} height={12} /></td>
                                        <td><SkeletonBone width={64} height={12} /></td>
                                    </tr>
                                ))}
                            </tbody>
                        </table>
                    </div>
                </article>
            </section>
        </div>
    )
}
