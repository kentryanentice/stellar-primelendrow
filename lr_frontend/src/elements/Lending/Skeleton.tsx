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

/* Movement labels vary a lot in length ("Deposit" vs "Collateral released"),
   so the first cell's bone varies with them rather than sitting at one width
   the real rows never have. */
const TX_KIND_WIDTHS = [64, 118, 84, 132, 74, 110, 96, 88]

export function TransactionRowsSkeleton({ rows = 8 }: { rows?: number }) {
    return (
        <>
            {Array.from({ length: rows }).map((_, i) => (
                <tr key={i}>
                    <td><SkeletonBone width={TX_KIND_WIDTHS[i % TX_KIND_WIDTHS.length]} height={14} /></td>
                    <td className='lending-ledger-amount'><SkeletonBone width={76} height={14} /></td>
                    <td><SkeletonBone width={74} height={18} radius={999} /></td>
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

/* Four labelled figures (Locked / Covering / Worth now / From wallet) and
   three movements: the shape a locked position actually has. A pending one is
   smaller, but it's also the case the record is rarely opened in — guessing at
   the common shape beats guessing at the small one and jumping on load. */
const CUSTODY_TERM_WIDTHS = [48, 62, 68, 78]
const CUSTODY_VALUE_WIDTHS = [86, 104, 118, 92]
const CUSTODY_MOVE_WIDTHS = [150, 196, 168]

/**
 * The collateral custody record (CollateralRecordCard) while GET
 * /loans/{id}/collateral is in flight. Composed inside the same
 * `.lending-custody` wrapper, grid and movement rows as the loaded card, so
 * the padding, gaps and column rules that decide its real dimensions are
 * shared rather than re-approximated — the panel opens at its final size and
 * the content fills in.
 *
 * Labelled rather than aria-hidden like the row fragments above: this replaces
 * a sentence that used to be read out ("Loading the collateral record…"), and
 * hiding it outright would leave a screen reader with silence where there was
 * an announcement. The bones themselves are empty, so the label is all there
 * is to read.
 */
export function CollateralRecordSkeleton() {
    return (
        <div className='lending-custody' role='status' aria-label='Loading the collateral record'>
            <div className='lending-custody-head'>
                <span className='lending-card-icon is-accent' />
                <h3><SkeletonBone width={124} height={14} /></h3>
                <span className='lending-custody-status'><SkeletonBone width={54} height={17} radius={999} /></span>
            </div>

            <dl className='lending-custody-grid'>
                {CUSTODY_TERM_WIDTHS.map((termWidth, i) => (
                    <div key={i}>
                        <dt><SkeletonBone width={termWidth} height={10} /></dt>
                        <dd><SkeletonBone width={CUSTODY_VALUE_WIDTHS[i]} height={14} /></dd>
                    </div>
                ))}
            </dl>

            <div className='lending-custody-price'>
                <p className='lending-muted' style={{ display: 'flex', flexDirection: 'column', gap: 5 }}>
                    <SkeletonBone width='88%' height={13} />
                    <SkeletonBone width='46%' height={13} />
                </p>
            </div>

            <ul className='lending-moves'>
                {CUSTODY_MOVE_WIDTHS.map((width, i) => (
                    <li key={i} className='lending-move'>
                        <span className='lending-move-what'><SkeletonBone width={width} height={13} /></span>
                        <span className='lending-move-link'><SkeletonBone width={92} height={12} /></span>
                    </li>
                ))}
            </ul>

            <p className='lending-muted' style={{ display: 'flex', flexDirection: 'column', gap: 5 }}>
                <SkeletonBone width='96%' height={13} />
                <SkeletonBone width='62%' height={13} />
            </p>
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
