import { RefreshCw, Users } from 'lucide-react'
import useAdminFunctions from '../functions/Admin/AdminFunctions'
import Carousel from '../elements/Admin/Carousel'
import Rail from '../elements/Admin/Rail'
import ReviewPanel from '../elements/Admin/ReviewPanel'

function Admin() {
    const admin = useAdminFunctions()
    const { mode, page, total, queueLoading, loadQueue } = admin

    return (
        <main className='admin-page'>
            <header className='admin-head'>
                <div>
                    <h1>Identity verification queue</h1>
                    <p>Review submitted documents and approve or reject each one.</p>
                    <span className='admin-pending-pill'>
                        <Users /> Pending ({total})
                    </span>
                </div>
                <button type='button' className='admin-refresh' onClick={() => loadQueue(page)} disabled={queueLoading}>
                    <RefreshCw className={queueLoading ? 'is-spinning' : ''} /> Refresh
                </button>
            </header>

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
        </main>
    )
}

export default Admin
