import { Wallet as WalletIcon } from 'lucide-react'
import { SkeletonBone } from '../Lending/Skeleton'

/**
 * Settings' cards never gate behind one page-level loading state (the hero
 * always has real data — AccessProvider blocks the route until the session
 * resolves) — each card fetches independently, so unlike Lending/Borrow/Pay
 * there's no single composed page skeleton here, just these per-card bodies
 * dropped into Settings.tsx's existing per-card loading branches. Reuses
 * SkeletonBone from Lending/Skeleton.tsx rather than duplicating it — see
 * that file for why bones compose directly into real layout classes instead
 * of a generic wrapper.
 */

/** Mirrors the credit-score arc gauge (Settings.tsx's `settings-score`
 *  block). The arc itself is an SVG whose rendered height tracks its own
 *  width responsively — approximated here with a fixed-height bone rather
 *  than reproducing that aspect-ratio math. */
export function CreditScoreSkeleton() {
    return (
        <div className='settings-score'>
            <div className='settings-score-arc'><SkeletonBone width='100%' height={135} radius={12} /></div>
            <div className='settings-score-axis'>
                <SkeletonBone width={12} height={10} />
                <SkeletonBone width={28} height={10} />
                <SkeletonBone width={40} height={10} />
                <SkeletonBone width={20} height={10} />
            </div>
            <p className='settings-muted' style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                <SkeletonBone width='95%' height={14} />
                <SkeletonBone width='60%' height={14} />
            </p>
        </div>
    )
}

/** Mirrors the identity-verification timeline. Modeled on the 3-step shape
 *  every non-"none" KYC status uses (verifying/approved/rejected all render
 *  three steps) — the same case detailLoading/kycLoading exists for. */
export function IdentityTimelineSkeleton() {
    return (
        <>
            <p className='settings-muted' style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                <SkeletonBone width='96%' height={14} />
                <SkeletonBone width='92%' height={14} />
                <SkeletonBone width='55%' height={14} />
            </p>
            <ol className='settings-timeline' aria-hidden='true'>
                {[0, 1, 2].map(i => (
                    <li key={i}>
                        <span className='settings-timeline-dot' />
                        {i < 2 && <span className='settings-timeline-line' />}
                        <div className='settings-timeline-body'>
                            <p className='settings-timeline-title'><SkeletonBone width={150} height={13} /></p>
                            <p className='settings-timeline-time'><SkeletonBone width={110} height={12} /></p>
                        </div>
                    </li>
                ))}
            </ol>
        </>
    )
}

/** WalletsCard's body only — one wallet row + the add-wallet form, no card-
 *  head. WalletsCard.tsx renders its own head unconditionally (above its
 *  `loading` branch), so its in-place loading state uses this directly
 *  instead of the section-wrapped WalletsCardSkeleton below, which would
 *  duplicate the head. Modeled on "has one connected wallet," the common
 *  case once scoreEligible — omits the "previously connected" history and
 *  "limit reached" states, both conditional on data this can't know yet. */
export function WalletsCardBody() {
    return (
        <>
            <ul className='settings-wallet-list' aria-hidden='true'>
                <li className='settings-wallet-row'>
                    <div className='settings-wallet-info'>
                        <div className='settings-wallet-info-top'><SkeletonBone width={140} height={13} /></div>
                        <div className='settings-wallet-meta-row'>
                            <SkeletonBone width={90} height={24} radius={999} />
                            <SkeletonBone width={170} height={18} />
                        </div>
                    </div>
                    <span className='settings-wallet-disconnect'><SkeletonBone width={78} height={12} /></span>
                </li>
            </ul>
            <div className='settings-wallet-add'>
                <div className='settings-wallet-label-input'><SkeletonBone width={100} height={13} /></div>
                <span className='settings-btn-primary'><SkeletonBone width={78} height={13} /></span>
            </div>
        </>
    )
}

/** WalletsCard's full section, head included — Settings.tsx's Suspense
 *  fallback for it (lazy-loaded to keep the wallet-connect SDKs out of the
 *  initial bundle). */
export function WalletsCardSkeleton() {
    return (
        <section className='settings-card settings-card-wallets'>
            <div className='settings-card-head'>
                <span className='settings-card-icon is-accent'><WalletIcon /></span>
                <h2>Wallets</h2>
            </div>
            <WalletsCardBody />
        </section>
    )
}
