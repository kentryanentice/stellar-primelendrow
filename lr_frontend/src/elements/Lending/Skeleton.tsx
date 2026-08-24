/**
 * One placeholder "bone" for a loading page. Deliberately not a generic
 * `<SkeletonCard>` — the real cards on Lending/Borrow/Pay (tile grids, tabbed
 * forms, ledger tables, pill lists) are too structurally different from each
 * other for one parameterized wrapper to fit all of them without either
 * losing dimensional accuracy or growing as many props as just writing the
 * JSX. Each page's skeleton composes this directly inside the *same* layout
 * classes the loaded page uses, so width/height here only has to match what
 * real content in that slot looks like — the surrounding grid/flex rules are
 * what make the overall shape match exactly.
 */
export function SkeletonBone({ width, height, radius }: { width: number | string; height: number; radius?: number }) {
    return (
        <span
            className='skeleton'
            aria-hidden='true'
            style={{ width, height, borderRadius: radius }}
        />
    )
}

/**
 * Row-shaped fragments below are deliberately separate from the three page
 * skeletons: each one is reused twice — once inside the full-page skeleton
 * (first load, shape unknown either way) and once inside the owning card's
 * *own* `loading` branch (its independent data hook re-fetching — most
 * visibly on a pager click, since page-level loading has already resolved by
 * then). Keeping one definition per row shape means both call sites move
 * together instead of drifting apart.
 */

const LEDGER_BADGE_WIDTHS = [128, 108, 98, 98, 108, 96]

export function LedgerRowsSkeleton({ rows = 6 }: { rows?: number }) {
    return (
        <>
            {Array.from({ length: rows }).map((_, i) => (
                <tr key={i}>
                    <td className='lending-ledger-amount'><SkeletonBone width={72} height={14} /></td>
                    <td><SkeletonBone width={LEDGER_BADGE_WIDTHS[i % LEDGER_BADGE_WIDTHS.length]} height={18} radius={999} /></td>
                    <td><SkeletonBone width={68} height={12} /></td>
                </tr>
            ))}
        </>
    )
}

const LOAN_TITLE_WIDTHS = [72, 68, 74, 70, 76, 66]

export function LoanRowsSkeleton({ rows = 6 }: { rows?: number }) {
    return (
        <>
            {Array.from({ length: rows }).map((_, i) => (
                <li key={i} className='lending-loan'>
                    <div className='lending-loan-summary'>
                        <div className='lending-loan-title'>
                            <b><SkeletonBone width={70} height={15} /></b>
                            <span><SkeletonBone width={LOAN_TITLE_WIDTHS[i % LOAN_TITLE_WIDTHS.length] * 2} height={12} /></span>
                        </div>
                        <span className='lending-loan-status'><SkeletonBone width={54} height={18} radius={999} /></span>
                    </div>
                </li>
            ))}
        </>
    )
}

export function PaymentRowsSkeleton({ rows = 6 }: { rows?: number }) {
    return (
        <>
            {Array.from({ length: rows }).map((_, i) => (
                <li key={i} className='lending-payment-row'>
                    <span className='lending-payment-row-icon' />
                    <span className='lending-payment-row-info'>
                        <b><SkeletonBone width={70} height={13} /></b>
                        <span><SkeletonBone width={90} height={11} /></span>
                    </span>
                    <span className='lending-payment-row-split'>
                        <span><SkeletonBone width={80} height={11} /></span>
                        <span className='is-interest'><SkeletonBone width={70} height={11} /></span>
                    </span>
                </li>
            ))}
        </>
    )
}

/** Same `.lending-pager` shape whether it's the first-load guess (page
 *  unknown) or a mid-pagination reload (page known, just not re-fetched
 *  yet) — callers with real page/totalPages on hand can render the live
 *  numbers instead if they'd rather; this is the shared "don't know yet"
 *  version. */
export function PagerSkeleton() {
    return (
        <div className='lending-pager' aria-hidden='true'>
            <span className='lending-pager-btn' />
            <SkeletonBone width={80} height={12} />
            <span className='lending-pager-btn' />
        </div>
    )
}

/**
 * Modeled on OpenLoanCard's *active*-loan shape (next payment + outstanding/
 * rate rows + Pay button + schedule toggle) rather than its slimmer pending-
 * loan shape or its `null` (no open loan) case — same reasoning PaySkeleton
 * uses for RepayCard: an open loan mid-repayment is the shape this slot most
 * often holds, and the one worth guessing at over rendering nothing.
 */
export function OpenLoanSkeleton() {
    return (
        <div className='lending-open-loan' aria-hidden='true'>
            <div className='lending-open-loan-head'>
                <span><SkeletonBone width={62} height={13} /></span>
                <span className='lending-loan-status'><SkeletonBone width={50} height={18} radius={999} /></span>
            </div>
            <div className='lending-open-loan-next'>
                <span className='lending-stat-label'><SkeletonBone width={86} height={11} /></span>
                <span className='lending-open-loan-amount'><SkeletonBone width={100} height={26} /></span>
                <span className='lending-muted'><SkeletonBone width='80%' height={13} /></span>
            </div>
            <div className='lending-open-loan-rows'>
                <div><span><SkeletonBone width={110} height={13} /></span><span><SkeletonBone width={70} height={13} /></span></div>
                <div><span><SkeletonBone width={70} height={13} /></span><span><SkeletonBone width={90} height={13} /></span></div>
            </div>
            <span className='lending-btn-primary'><SkeletonBone width={110} height={15} /></span>
            <span className='lending-open-loan-toggle'><SkeletonBone width={90} height={13} /></span>
        </div>
    )
}
