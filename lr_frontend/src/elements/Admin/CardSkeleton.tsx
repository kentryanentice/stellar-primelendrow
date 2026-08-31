import { SkeletonBone } from '../Lending/Skeleton'

/**
 * Mirrors Card.tsx's loaded shape — same .admin-card/.admin-card-head/
 * .admin-card-photos/.admin-card-body wrapper classes as the real card, so
 * dimensions match by construction rather than by hand-measured pixels (see
 * Lending/Skeleton.tsx, which this reuses SkeletonBone from, for why bones
 * compose directly into real layout classes instead of a generic wrapper).
 */
function CardSkeleton() {
    return (
        <div className='admin-card' aria-hidden='true'>
            <div className='admin-card-head'>
                <span className='admin-card-type'><SkeletonBone width={90} height={11} /></span>
                <span className='admin-card-time'><SkeletonBone width={46} height={11} /></span>
            </div>

            <div className='admin-card-photos'>
                <div className='admin-card-photo'><SkeletonBone width='100%' height={148} radius={14} /></div>
                <div className='admin-card-photo'><SkeletonBone width='100%' height={148} radius={14} /></div>
            </div>

            <div className='admin-card-body'>
                <div className='admin-card-score-row'>
                    <div className='admin-card-score'>
                        <span className='admin-card-score-value'><SkeletonBone width={50} height={26} /></span>
                        <span className='admin-card-score-label'><SkeletonBone width={64} height={10} /></span>
                    </div>
                    <span className='admin-card-verdict'><SkeletonBone width={86} height={13} /></span>
                </div>
                <div className='admin-card-track' />

                <div className='admin-card-badges'>
                    <span className='admin-card-badge'><SkeletonBone width={120} height={13} /></span>
                </div>

                <span className='admin-card-info'><SkeletonBone width={70} height={14} /></span>
            </div>
        </div>
    )
}

export default CardSkeleton
