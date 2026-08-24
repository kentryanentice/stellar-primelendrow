import { lazy, Suspense } from 'react'
import useLendingPool from '../functions/Lending/useLendingPool'
import useLoanHistory from '../functions/Lending/useLoanHistory'
import useLoans from '../functions/Lending/useLoans'
import useBorrowForm from '../functions/Lending/useBorrowForm'
import { pesosCompact } from '../functions/Lending/money'
import EligibilityCard from '../elements/Lending/EligibilityCard'
import OpenLoanCard from '../elements/Lending/OpenLoanCard'
import { OpenLoanSkeleton } from '../elements/Lending/Skeleton'
import BorrowSkeleton, { BorrowCardSkeleton, LoanHistoryCardSkeleton } from '../elements/Lending/BorrowSkeleton'
import type { PolicyParams } from '../functions/Lending/types'

// Same split as Lending: BorrowCard pulls the wallet kit, LoanHistoryCard is
// lazy alongside it so the page shell paints first.
const BorrowCard = lazy(() => import('../elements/Lending/BorrowCard'))
const LoanHistoryCard = lazy(() => import('../elements/Lending/LoanHistoryCard'))

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
 * A sticky "Your credit" rail (score, the one loan currently open if any,
 * borrowing history) next to a scrolling column for applying (three
 * products) and browsing loan history. Shares the pool read with the
 * Lending page (GET /pool) since the quote/policy numbers this page shows
 * come from the same engine data — this page just doesn't render the
 * pool/funds/guarantor cards that live over there.
 */
function Borrow() {
    const { data, loading, error, refresh } = useLendingPool()
    const history = useLoanHistory()
    const loans = useLoans()

    // Applying, locking, or resuming a lock can change the pool's badge
    // totals (GET /pool), the paginated history list, and which loan (if
    // any) is open — refresh all three together so nothing on the page
    // disagrees with anything else.
    const handleChanged = () => {
        refresh()
        history.refresh()
        loans.refresh()
    }

    // Lifted to the page (not owned inside BorrowCard) so EligibilityCard can
    // read the same live product/quote instead of racing a second debounced
    // quote request of its own.
    const form = useBorrowForm(data?.params.policy ?? FALLBACK_POLICY, handleChanged)

    const openLoan = loans.loans.find(l => l.status === 'pending' || l.status === 'active') ?? null

    // Lifetime stats: GET /loans (unlike the paginated /loans/history) has no
    // status filter and no LIMIT — it's the caller's complete history, so
    // these sums are correct across every loan they've ever had, not just
    // whatever page happens to be loaded.
    const repaidCount = loans.loans.filter(l => l.status === 'closed').length
    // disbursed_at gates this so a 'pending'/'declined'/'cancelled' loan
    // (money never actually moved) doesn't inflate "borrowed to date"
    const borrowedTotal = loans.loans.filter(l => l.disbursed_at !== null).reduce((sum, l) => sum + l.principal, 0)
    const hasLatePayment = loans.loans.some(l => l.schedule.some(row => row.status === 'late'))

    if (loading) {
        return <main className='lending-page'><BorrowSkeleton /></main>
    }

    if (error || !data) {
        return (
            <main className='lending-page'>
                <header className='lending-head'>
                    <p className='lending-eyebrow'>Borrowing</p>
                    <h1>Your credit</h1>
                </header>
                <section className='lending-card'>
                    <p className='lending-muted'>Couldn’t load the lending pool. Please try again later.</p>
                    <button type='button' className='lending-btn' onClick={refresh}>Retry</button>
                </section>
            </main>
        )
    }

    return (
        <main className='lending-page'>
            <header className='lending-head lending-head-with-pill'>
                <div>
                    <p className='lending-eyebrow'>Borrowing</p>
                    <h1>Your credit &amp; loans</h1>
                </div>
                {history.total > 0 && <p className='lending-muted'>{history.total} {history.total === 1 ? 'loan' : 'loans'}</p>}
            </header>

            <div className='lending-borrow-layout'>
                <aside className='lending-borrow-sidebar'>
                    <EligibilityCard data={data} />
                    {loans.loading ? <OpenLoanSkeleton /> : !loans.error && <OpenLoanCard loan={openLoan} />}
                    {!loans.loading && !loans.error && loans.loans.length > 0 && (
                        <div className='lending-lifetime-stats'>
                            <span className='lending-stat-label'>Borrowing lifetime stats</span>
                            <div className='lending-lifetime-pills'>
                                <span className='lending-lifetime-pill'>{repaidCount} {repaidCount === 1 ? 'loan' : 'loans'} repaid</span>
                                <span className='lending-lifetime-pill'>{pesosCompact(borrowedTotal)} borrowed</span>
                                <span className={`lending-lifetime-pill${hasLatePayment ? ' is-warn' : ' is-good'}`}>
                                    {hasLatePayment ? 'Has late payments' : 'No late payments'}
                                </span>
                            </div>
                        </div>
                    )}
                </aside>

                <div className='lending-borrow-main'>
                    <Suspense fallback={<BorrowCardSkeleton />}>
                        <BorrowCard data={data} form={form} openLoan={openLoan} />
                    </Suspense>
                    <Suspense fallback={<LoanHistoryCardSkeleton />}>
                        <LoanHistoryCard data={data} history={history} onChanged={handleChanged} />
                    </Suspense>
                </div>
            </div>
        </main>
    )
}

export default Borrow
