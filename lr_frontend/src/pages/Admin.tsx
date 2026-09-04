import { useState } from 'react'
import { RefreshCw, Users } from 'lucide-react'
import useAdminFunctions from '../functions/Admin/AdminFunctions'
import useAdminLending from '../functions/Admin/useAdminLending'
import Carousel from '../elements/Admin/Carousel'
import Rail from '../elements/Admin/Rail'
import ReviewPanel from '../elements/Admin/ReviewPanel'
import LoansPanel from '../elements/Admin/LoansPanel'

type Section = 'kyc' | 'loans'

/**
 * Two consoles behind one header. Identity verification is the original one;
 * lending was added alongside it rather than inside it because the two share
 * nothing but the operator — different data, different powers, and mixing a
 * "default this loan" button into a document review queue would be a way to
 * click it by accident.
 */
function Admin() {
    const admin = useAdminFunctions()
    const { mode, page, total, queueLoading, loadQueue } = admin
    const lending = useAdminLending()

    const [section, setSection] = useState<Section>('kyc')
    const onKyc = section === 'kyc'

    return (
        <main className='admin-page'>
            <header className='admin-head'>
                <div>
                    <h1>{onKyc ? 'Identity verification queue' : 'Lending operations'}</h1>
                    <p>
                        {onKyc
                            ? 'Review submitted documents and approve or reject each one.'
                            : 'The loan book, defaults, and the vault movements waiting for your key.'}
                    </p>
                    {onKyc ? (
                        <span className='admin-pending-pill'>
                            <Users /> Pending ({total})
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
                    onClick={() => (onKyc ? loadQueue(page) : void lending.refresh())}
                    disabled={onKyc ? queueLoading : lending.loading}
                >
                    <RefreshCw className={(onKyc ? queueLoading : lending.loading) ? 'is-spinning' : ''} /> Refresh
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
                    className={`lending-tab${onKyc ? '' : ' is-active'}`}
                    onClick={() => setSection('loans')}
                >
                    Lending
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
            ) : (
                <LoansPanel lending={lending} />
            )}
        </main>
    )
}

export default Admin
