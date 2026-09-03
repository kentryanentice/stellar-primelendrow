import { lazy, Suspense } from 'react'
import useLendingPool from '../functions/Lending/useLendingPool'
import useDeposits from '../functions/Lending/useDeposits'
import useLoans from '../functions/Lending/useLoans'
import useTransactions from '../functions/Lending/useTransactions'
import { pesos } from '../functions/Lending/money'

import PoolOverviewCard from '../elements/Lending/PoolOverviewCard'
import RateTiersCard from '../elements/Lending/RateTiersCard'
import YourDepositsCard from '../elements/Lending/YourDepositsCard'
import TransactionsCard from '../elements/Lending/TransactionsCard'
import GuarantorCard from '../elements/Lending/GuarantorCard'
import LendingSkeleton, { ManageFundsCardSkeleton } from '../elements/Lending/LendingSkeleton'

// ManageFundsCard pulls the PayPal SDK bootstrap — split out so the page
// shell paints first (same rationale as Settings' lazy WalletsCard).
// Borrowing and loan tracking live on the separate /borrow page.
const ManageFundsCard = lazy(() => import('../elements/Lending/ManageFundsCard'))

/**
 * The lending pool page: pool stats and rate tiers full-width up top, then a
 * sticky money rail (balance + deposit/withdraw) that stays in reach on the
 * left next to a ledger-dominant right column (deposit lots, guarantee
 * requests) that scrolls under it. Every number on it comes from the engine
 * (GET /pool, GET /pool/deposits, /guarantors/invites) — this page renders
 * and records intent, it never does money math. Borrowing and loan tracking
 * live on the separate /borrow page.
 */
function Lending() {
    const { data, loading, error, refresh } = useLendingPool()
    const deposits = useDeposits()
    const loans = useLoans()
    const transactions = useTransactions()

    // A deposit/withdraw changes the pool's badge totals (GET /pool), the lot
    // list (POST /pool/deposits) and the movement history (POST
    // /pool/transactions) — refresh all three together so no two cards on the
    // page show numbers that disagree with each other.
    const handleChanged = () => {
        refresh()
        deposits.refresh()
        transactions.refresh()
    }

    // At most one pending-or-active loan per borrower (same DB fact the
    // Borrow page's OpenLoanCard relies on) — this is deliberately the
    // caller's own loan count, never data.pool.active_loans (pool-wide),
    // so it never gets paired with the personal me.collateral figure below.
    const openLoanCount = loans.loans.filter(l => l.status === 'pending' || l.status === 'active').length

    if (loading) {
        return <main className='lending-page'><LendingSkeleton /></main>
    }

    if (error || !data) {
        return (
            <main className='lending-page'>
                <header className='lending-head'>
                    <p className='lending-eyebrow'>Lending</p>
                    <h1>Lend to the pool</h1>
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
                    <p className='lending-eyebrow'>Lending</p>
                    <h1>Lend to the pool</h1>
                </div>
                {openLoanCount > 0 && (
                    <p className='lending-muted'>
                        {openLoanCount} active {openLoanCount === 1 ? 'loan' : 'loans'}
                        {data.me.collateral > 0 && <> · {pesos(data.me.collateral)} of your deposit is backing it</>}
                    </p>
                )}
            </header>

            <PoolOverviewCard data={data} />
            <RateTiersCard data={data} />

            <div className='lending-rail-layout'>
                <aside className='lending-rail'>
                    <Suspense fallback={<ManageFundsCardSkeleton />}>
                        <ManageFundsCard data={data} onChanged={handleChanged} />
                    </Suspense>
                </aside>

                <div className='lending-main'>
                    <YourDepositsCard deposits={deposits} />
                    <TransactionsCard transactions={transactions} />
                    <GuarantorCard onChanged={refresh} />
                </div>
            </div>
        </main>
    )
}

export default Lending
