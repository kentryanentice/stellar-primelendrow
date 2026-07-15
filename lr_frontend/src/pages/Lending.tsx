import { lazy, Suspense } from 'react'
import useLendingPool from '../functions/Lending/useLendingPool'
import useDeposits from '../functions/Lending/useDeposits'

import PoolOverviewCard from '../elements/Lending/PoolOverviewCard'
import RateTiersCard from '../elements/Lending/RateTiersCard'
import YourDepositsCard from '../elements/Lending/YourDepositsCard'
import GuarantorCard from '../elements/Lending/GuarantorCard'

// ManageFundsCard pulls the PayPal SDK bootstrap — split out so the page
// shell paints first (same rationale as Settings' lazy WalletsCard).
// Borrowing and loan tracking live on the separate /borrow page.
const ManageFundsCard = lazy(() => import('../elements/Lending/ManageFundsCard'))

function CardFallback({ title }: { title: string }) {
    return (
        <section className='lending-card'>
            <div className='lending-card-head'><h2>{title}</h2></div>
            <p className='lending-muted'>Loading…</p>
        </section>
    )
}

/**
 * The lending pool page: pool overview, rate tiers, manage funds (deposit /
 * withdraw), your deposit lots, and guarantee invitations. Every number on it
 * comes from the engine (GET /pool, GET /pool/deposits, /guarantors/invites)
 * — this page renders and records intent, it never does money math.
 * Borrowing and loan tracking live on the separate /borrow page.
 */
function Lending() {
    const { data, loading, error, refresh } = useLendingPool()
    const deposits = useDeposits()

    // A deposit/withdraw changes both the pool's badge totals (GET /pool) and
    // the lot list (GET /pool/deposits) — refresh both together so the two
    // cards never show numbers that disagree with each other.
    const handleChanged = () => {
        refresh()
        deposits.refresh()
    }

    return (
        <main className='lending-page'>
            <header className='lending-head lending-head-with-pill'>
                <div>
                    <p className='lending-eyebrow'>Lending</p>
                    <h1>Lend to the pool</h1>
                    <p>Your deposits fund the community. Borrowers repay with interest that flows back to lenders.</p>
                </div>
                {data && (
                    <span className='lending-pill is-accent lending-active-pill'>
                        {data.pool.active_loans} active {data.pool.active_loans === 1 ? 'loan' : 'loans'}
                    </span>
                )}
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
                <>
                    <div className='lending-columns'>
                        <div className='lending-column'>
                            <PoolOverviewCard data={data} />
                            <RateTiersCard data={data} />
                        </div>
                        <div className='lending-column'>
                            <Suspense fallback={<CardFallback title='Manage funds' />}>
                                <ManageFundsCard data={data} onChanged={handleChanged} />
                            </Suspense>
                            <YourDepositsCard deposits={deposits} />
                        </div>
                    </div>
                    <GuarantorCard onChanged={refresh} />
                </>
            )}
        </main>
    )
}

export default Lending
