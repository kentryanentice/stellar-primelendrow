import { useState } from 'react'
import { Coins, RefreshCw, Users } from 'lucide-react'
import useAdminFunctions from '../functions/Admin/AdminFunctions'
import useAdminLending from '../functions/Admin/useAdminLending'
import useAdminInterest from '../functions/Admin/useAdminInterest'
import Carousel from '../elements/Admin/Carousel'
import Rail from '../elements/Admin/Rail'
import ReviewPanel from '../elements/Admin/ReviewPanel'
import LoansPanel from '../elements/Admin/LoansPanel'
import InterestPanel from '../elements/Admin/InterestPanel'

type Section = 'kyc' | 'loans' | 'interest'

/**
 * Three consoles behind one header. The interest history sits beside the loan
 * book rather than inside it: it is read-only and pool-wide (one payment
 * reaches every depositor), while the loan book is per loan and holds the
 * controls that move money.
 *
 * The first two consoles: Identity verification is the original one;
 * lending was added alongside it rather than inside it because the two share
 * nothing but the operator — different data, different powers, and mixing a
 * "default this loan" button into a document review queue would be a way to
 * click it by accident.
 */
function Admin() {
    const admin = useAdminFunctions()
    const { mode, page, total, queueLoading, loadQueue } = admin
    const lending = useAdminLending()
    const interest = useAdminInterest()

    const [section, setSection] = useState<Section>('kyc')
    const onKyc = section === 'kyc'
    const onInterest = section === 'interest'
    const busy = onKyc ? queueLoading : onInterest ? interest.loading : lending.loading

    return (
        <main className='admin-page'>
            <header className='admin-head'>
                <div>
                    <h1>{onKyc ? 'Identity verification queue' : onInterest ? 'Interest history' : 'Lending operations'}</h1>
                    <p>
                        {onKyc
                            ? 'Review submitted documents and approve or reject each one.'
                            : onInterest
                                ? 'Every repayment’s interest: who received it, how much, and why.'
                                : 'The loan book, defaults, and the vault movements waiting for your key.'}
                    </p>
                    {onKyc ? (
                        <span className='admin-pending-pill'>
                            <Users /> Pending ({total})
                        </span>
                    ) : onInterest ? (
                        <span className='admin-pending-pill'>
                            <Coins /> {interest.total} {interest.total === 1 ? 'repayment' : 'repayments'}
                        </span>
                    ) : (
                        <span className='admin-pending-pill'>
                            <Users /> {lending.total} {lending.total === 1 ? 'loan' : 'loans'}
                            {lending.actions.length > 0 && ` · ${lending.actions.length} awaiting signature`}
                        </span>
                    )}
                </div>
                <button
                    type='button'
                    className='admin-refresh'
                    onClick={() => (onKyc ? loadQueue(page) : onInterest ? void interest.refresh() : void lending.refresh())}
                    disabled={busy}
                >
                    <RefreshCw className={busy ? 'is-spinning' : ''} /> Refresh
                </button>
            </header>

            <div className='admin-sections'>
                <button
                    type='button'
                    className={`lending-tab${onKyc ? ' is-active' : ''}`}
                    onClick={() => setSection('kyc')}
                >
                    Identity
                </button>
                <button
                    type='button'
                    className={`lending-tab${section === 'loans' ? ' is-active' : ''}`}
                    onClick={() => setSection('loans')}
                >
                    Lending
                </button>
                <button
                    type='button'
                    className={`lending-tab${onInterest ? ' is-active' : ''}`}
                    onClick={() => setSection('interest')}
                >
                    Interest
                </button>
            </div>

            {onKyc ? (
                <div className={`admin-stage${mode === 'browse' ? ' is-browse' : ''}`}>
                    {mode === 'browse' ? (
                        <Carousel {...admin} />
                    ) : (
                        <div className='admin-review-layout'>
                            <Rail {...admin} />
                            <ReviewPanel {...admin} />
                        </div>
                    )}
                </div>
            ) : onInterest ? (
                <InterestPanel interest={interest} />
            ) : (
                <LoansPanel lending={lending} />
            )}
        </main>
    )
}

export default Admin
