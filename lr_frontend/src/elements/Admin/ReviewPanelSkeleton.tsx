import { SkeletonBone } from '../Lending/Skeleton'

/**
 * Mirrors ReviewPanel.tsx's loaded shape — same .admin-panel-head/.admin-
 * compare/.admin-signals/.admin-detail-fields/.admin-panel-actions wrapper
 * classes as the real panel, so dimensions match by construction (see
 * Lending/Skeleton.tsx for why bones compose directly into real layout
 * classes rather than a generic wrapper).
 *
 * Doesn't include the "Reviewed" field in .admin-detail-fields (conditional
 * on `detail.reviewed_at`, which a still-pending submission never has) —
 * modeled on the pending-submission shape, the case detailLoading exists
 * for in the first place.
 */
function ReviewPanelSkeleton() {
    return (
        <>
            <div className='admin-panel-head'>
                <div className='admin-panel-head-text'>
                    <span className='admin-panel-eyebrow'><SkeletonBone width={110} height={11} /></span>
                    <div className='admin-panel-name-row'>
                        <h2><SkeletonBone width={200} height={21} /></h2>
                        <span className='admin-panel-type'><SkeletonBone width={140} height={12} /></span>
                    </div>
                </div>
                <span className='admin-panel-close'><SkeletonBone width={18} height={18} /></span>
                <div className='admin-panel-accent-line' />
            </div>

            <div className='admin-panel-scroll'>
                <div className='admin-compare'>
                    <div className='admin-compare-photo'><SkeletonBone width='100%' height={232} radius={16} /></div>

                    <div className='admin-compare-ring-wrap'>
                        <SkeletonBone width={132} height={132} radius={999} />
                        <span className='admin-verdict'><SkeletonBone width={90} height={13} /></span>
                    </div>

                    <div className='admin-compare-id'>
                        <div className='admin-compare-id-head'>
                            <SkeletonBone width={26} height={26} radius={999} />
                            <div>
                                <p className='admin-compare-id-eyebrow'><SkeletonBone width={90} height={8} /></p>
                                <p className='admin-compare-id-type'><SkeletonBone width={130} height={12} /></p>
                            </div>
                        </div>
                        <div className='admin-compare-id-body'>
                            <div className='admin-compare-id-photo'><SkeletonBone width={78} height={98} radius={6} /></div>
                            <div className='admin-compare-id-fields'>
                                {[80, 100, 120].map((w, i) => (
                                    <div key={i}>
                                        <span><SkeletonBone width={60} height={8} /></span>
                                        <p><SkeletonBone width={w} height={13} /></p>
                                    </div>
                                ))}
                            </div>
                        </div>
                    </div>
                </div>

                <div className='admin-signals'>
                    {[0, 1].map(i => (
                        <div key={i} className='admin-signal'>
                            <div className='admin-signal-head'>
                                <span><SkeletonBone width={64} height={11} /></span>
                                <SkeletonBone width={8} height={8} radius={999} />
                            </div>
                            <p className='admin-signal-value'><SkeletonBone width={54} height={16} /></p>
                        </div>
                    ))}
                </div>

                <div className='admin-detail-fields'>
                    {[70, 110, 150, 100].map((w, i) => (
                        <div key={i}>
                            <dt><SkeletonBone width={80} height={10} /></dt>
                            <dd><SkeletonBone width={w} height={13} /></dd>
                        </div>
                    ))}
                </div>
            </div>

            <div className='admin-panel-actions'>
                <div className='admin-actions-row is-decide'>
                    <span className='admin-btn-approve'><SkeletonBone width={70} height={15} /></span>
                    <span className='admin-btn-reject'><SkeletonBone width={60} height={15} /></span>
                </div>
            </div>
        </>
    )
}

export default ReviewPanelSkeleton
