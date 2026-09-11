import { pesos, pesosCompact, rate } from '../../functions/Lending/money'
import { PRODUCT_LABEL, type Loan, type PoolResponse } from '../../functions/Lending/types'
import { SkeletonBone } from '../Lending/Skeleton'

type Props = {
    data: PoolResponse
    /** The active or settling loan, if any — the one that's actually owed. */
    loan: Loan | null
    pendingLoan: Loan | null
    loansLoading: boolean
}

/**
 * The four numbers a member checks first: what they hold in the pool, what of
 * it they can take out, what's tied up behind loans, and what they owe. The
 * first three are the engine's badge totals from GET /pool; the last is the
 * open loan's own figure — its arrears while settling a default, since a
 * settlement doesn't re-charge the whole schedule (see RepayCard).
 */
function SummaryTiles({ data, loan, pendingLoan, loansLoading }: Props) {
    const { me } = data
    const locked = me.lent + me.collateral + me.pledged
    const lockedParts = [
        me.lent > 0 && `${pesosCompact(me.lent)} funding`,
        me.collateral > 0 && `${pesosCompact(me.collateral)} backing`,
        me.pledged > 0 && `${pesosCompact(me.pledged)} pledged`,
    ].filter(Boolean)
    const settling = loan?.status === 'reconciling'

    return (
        <div className='dash-tiles'>
            <div className='dash-tile'>
                <span className='lending-stat-label'>In the pool</span>
                <span className='dash-tile-value'>{pesos(me.available + locked)}</span>
                <span className='dash-tile-sub'>Your deposits, free or locked</span>
            </div>
            <div className='dash-tile'>
                <span className='lending-stat-label'>Withdrawable</span>
                <span className='dash-tile-value is-good'>{pesos(me.available)}</span>
                <span className='dash-tile-sub'>Available right now</span>
            </div>
            <div className='dash-tile'>
                <span className='lending-stat-label'>Locked in loans</span>
                <span className='dash-tile-value is-warn'>{pesos(locked)}</span>
                <span className='dash-tile-sub'>{lockedParts.length > 0 ? lockedParts.join(' · ') : 'Nothing locked'}</span>
            </div>
            <div className='dash-tile'>
                <span className='lending-stat-label'>You owe</span>
                {loansLoading ? (
                    <>
                        <span className='dash-tile-value'><SkeletonBone width={110} height={26} /></span>
                        <span className='dash-tile-sub'><SkeletonBone width={140} height={12} /></span>
                    </>
                ) : (
                    <>
                        <span className='dash-tile-value'>
                            {pesos(loan ? (settling ? loan.arrears : loan.principal_outstanding) : 0)}
                        </span>
                        <span className='dash-tile-sub'>
                            {loan
                                ? settling
                                    ? 'Settling a defaulted loan'
                                    : `${PRODUCT_LABEL[loan.product]} · ${rate(loan.rate_bps)} · ${loan.term_months} mo`
                                : pendingLoan
                                    ? `${pesosCompact(pendingLoan.principal)} loan awaiting setup`
                                    : 'No open loan'}
                        </span>
                    </>
                )}
            </div>
        </div>
    )
}

export default SummaryTiles
