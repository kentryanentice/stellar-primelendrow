import { Link, useLocation, useParams, useSearchParams } from 'react-router-dom'
import { ArrowLeft } from 'lucide-react'
import Theme from '../elements/Theme'
import RecordsList from '../elements/Records/RecordsList'
import RecordDetail from '../elements/Records/RecordDetail'
import { RecordDetailSkeleton } from '../elements/Records/RecordsSkeleton'
import {
    usePublicLoan,
    usePublicLoans,
    type ProductFilter,
    type StatusFilter,
} from '../functions/Lending/publicLoans'

const STATUSES: StatusFilter[] = ['', 'pending', 'active', 'closed', 'defaulted', 'declined', 'cancelled']
const PRODUCTS: ProductFilter[] = ['', 'deposit_backed', 'xlm_collateral', 'guarantor']

/** The list, with its page and filters kept in the URL so they survive a
 *  refresh, a shared link, and the trip into a loan and back. */
function LoanBook() {
    const [params, setParams] = useSearchParams()
    const rawStatus = params.get('status') ?? ''
    const rawProduct = params.get('product') ?? ''
    const status = (STATUSES as string[]).includes(rawStatus) ? rawStatus as StatusFilter : ''
    const product = (PRODUCTS as string[]).includes(rawProduct) ? rawProduct as ProductFilter : ''
    const page = Math.max(1, Number.parseInt(params.get('page') ?? '1', 10) || 1)
    const result = usePublicLoans(page, status, product)

    return (
        <RecordsList
            result={result}
            status={status}
            product={product}
            onFilter={next => {
                const merged = {
                    status: next.status ?? status,
                    product: next.product ?? product,
                    page: next.page ?? page,
                }
                const query = new URLSearchParams()
                if (merged.status) query.set('status', merged.status)
                if (merged.product) query.set('product', merged.product)
                if (merged.page > 1) query.set('page', String(merged.page))
                setParams(query)
            }}
        />
    )
}

function LoanRecord({ loanId }: { loanId: string }) {
    const { data, loading, error, notFound } = usePublicLoan(loanId)

    if (notFound) {
        return (
            <section className='lending-card'>
                <p className='lending-muted'>There’s no loan with that reference.</p>
            </section>
        )
    }
    if (loading) return <RecordDetailSkeleton />
    if (error || !data) {
        return (
            <section className='lending-card'>
                <p className='lending-muted'>Couldn’t load this loan. Please try again later.</p>
            </section>
        )
    }
    return <RecordDetail record={data} />
}

/**
 * The public loan book (/records, /records/:loanId). No account needed and
 * nobody named: every application ever made, what backed it, how it was
 * repaid and where the interest went. Sits outside the app shell, under the
 * same header as the landing page, because most of the people reading it are
 * not signed in.
 */
function Records() {
    const { loanId } = useParams()
    const location = useLocation()
    // The list's query, handed over by the row that was clicked, so "back"
    // returns to the same page and filter rather than the top of the book.
    const from = (location.state as { from?: string } | null)?.from ?? ''

    return (
        <div className='records'>
            <img className='records-bg' src='/pictures/hero-bg.png' alt='' />
            <Theme />
            <main className='lending-page'>
                <header className='lending-head'>
                    <p className='lending-eyebrow'>Public records</p>
                    <h1>{loanId ? 'Loan record' : 'Loan book'}</h1>
                    <p>
                        Every loan application on PrimeLendRow — approved, declined, cancelled, repaid or defaulted — with
                        what backed it, every repayment and where its interest went. No member is identified.
                    </p>
                </header>

                {loanId && (
                    <Link to={`/records${from}`} className='records-back'>
                        <ArrowLeft aria-hidden='true' /> All loans
                    </Link>
                )}

                {loanId ? <LoanRecord key={loanId} loanId={loanId} /> : <LoanBook />}
            </main>
        </div>
    )
}

export default Records
