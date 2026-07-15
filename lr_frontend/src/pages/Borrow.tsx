import { lazy, Suspense } from 'react'
import useLendingPool from '../functions/Lending/useLendingPool'
import useLoanHistory from '../functions/Lending/useLoanHistory'
import useBorrowForm from '../functions/Lending/useBorrowForm'
import EligibilityCard from '../elements/Lending/EligibilityCard'
import type { PolicyParams } from '../functions/Lending/types'

// Same split as Lending: BorrowCard pulls the wallet kit, LoanHistoryCard is
// lazy alongside it so the page shell paints first.
const BorrowCard = lazy(() => import('../elements/Lending/BorrowCard'))
const LoanHistoryCard = lazy(() => import('../elements/Lending/LoanHistoryCard'))

function CardFallback({ title }: { title: string }) {
    return (
        <section className='lending-card'>
            <div className='lending-card-head'><h2>{title}</h2></div>
            <p className='lending-muted'>Loading…</p>
        </section>
    )
}

// Stands in only until GET /pool resolves: useBorrowForm needs a PolicyParams
// shape to initialize (term defaults to its min; validation reads min_loan),
// and hooks can't be called conditionally once data arrives.
const FALLBACK_POLICY: PolicyParams = {
    bands: [],
    deposit_ltv_pct: 0,
    xlm_min_collateral_pct: 0,
    xlm_liquidation_pct: 0,
    guarantor_cap_multiple: 1,
    guarantors_max: 1,
    term_months: { min: 3, max: 12 },
    min_deposit: 0,
    min_loan: 0,
    interest_split: { savers: 0, platform: 0, reserve: 0 },
}

/**
 * Apply for a loan (three products) and browse loan history. Shares the pool
 * read with the Lending page (GET /pool) since the quote/policy numbers this
 * page shows come from the same engine data — this page just doesn't render
 * the pool/funds/guarantor cards that live over there.
 */
function Borrow() {
    const { data, loading, error, refresh } = useLendingPool()
    const history = useLoanHistory()

    // Applying, locking, or resuming a lock can all change both the pool's
    // badge totals (GET /pool) and the loan list (POST /loans/history) —
    // refresh both together so the two cards never disagree.
    const handleChanged = () => {
        refresh()
        history.refresh()
    }

    // Lifted to the page (not owned inside BorrowCard) so EligibilityCard can
    // read the same live product/quote instead of racing a second debounced
    // quote request of its own.
    const form = useBorrowForm(data?.params.policy ?? FALLBACK_POLICY, handleChanged)

    return (
        <main className='lending-page'>
            <header className='lending-head'>
                <p className='lending-eyebrow'>Borrowing</p>
                <h1>Apply for a loan</h1>
                <p>Choose how you’ll back the loan. Your rate and limit depend on your credit tier.</p>
            </header>

            {loading ? (
                <section className='lending-card'>
                    <p className='lending-muted'>Loading the pool…</p>
                </section>
            ) : error || !data ? (
                <section className='lending-card'>
                    <p className='lending-muted'>Couldn’t load the lending pool. Please try again later.</p>
                    <button type='button' className='lending-btn' onClick={refresh}>Retry</button>
                </section>
            ) : (
                <div className='lending-columns'>
                    <div className='lending-column'>
                        <Suspense fallback={<CardFallback title='Apply for a loan' />}>
                            <BorrowCard data={data} form={form} />
                        </Suspense>
                    </div>
                    <div className='lending-column'>
                        <EligibilityCard data={data} form={form} />
                        <Suspense fallback={<CardFallback title='Your loans' />}>
                            <LoanHistoryCard data={data} history={history} onChanged={handleChanged} />
                        </Suspense>
                    </div>
                </div>
            )}
        </main>
    )
}

export default Borrow
