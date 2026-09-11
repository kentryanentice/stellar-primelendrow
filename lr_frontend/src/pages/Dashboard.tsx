import { useState } from 'react'
import { useSession } from '../providers/useSession'
import useLendingPool from '../functions/Lending/useLendingPool'
import useLoans from '../functions/Lending/useLoans'
import useTransactions from '../functions/Lending/useTransactions'
import usePayments from '../functions/Lending/usePayments'
import { formatDate } from '../functions/Lending/money'
import { loanStanding, recentActivity } from '../functions/Dashboard/overview'

import SummaryTiles from '../elements/Dashboard/SummaryTiles'
import BalanceCard from '../elements/Dashboard/BalanceCard'
import TierLimitsCard from '../elements/Dashboard/TierLimitsCard'
import NextPaymentCard from '../elements/Dashboard/NextPaymentCard'
import RecentActivityCard from '../elements/Dashboard/RecentActivityCard'
import PoolStrip from '../elements/Dashboard/PoolStrip'
import DashboardSkeleton from '../elements/Dashboard/DashboardSkeleton'
import VerifyPrompt from '../elements/Dashboard/VerifyPrompt'
import EligibilityCard from '../elements/Lending/EligibilityCard'
import GuarantorCard from '../elements/Lending/GuarantorCard'

/**
 * The member's whole position on one screen: what they hold, what's locked,
 * what they owe and when, their credit standing, and the latest movements.
 * Reads the same engine endpoints the Lend, Borrow and Pay pages do (GET
 * /pool, GET /loans, POST /pool/transactions, POST /loans/payments) and does
 * no money math of its own — each card links through to the page where the
 * action actually happens.
 */
function Overview() {
    const { data, loading, error, refresh } = useLendingPool()
    const loans = useLoans()
    const transactions = useTransactions()
    const payments = usePayments()
    // read once on mount, not per render — a render shouldn't depend on the clock
    const [today] = useState(() => formatDate(Date.now() / 1000))

    // Accepting a guarantee request freezes part of the caller's deposit, which
    // moves both the balance figures (GET /pool) and the movement feed.
    const handleChanged = () => {
        refresh()
        transactions.refresh()
    }

    // Same selection the Pay page makes: a `reconciling` loan is a reopened
    // default that pays through the ordinary repay flow, so it's the loan owed.
    const loan = loans.loans.find(l => l.status === 'active' || l.status === 'reconciling') ?? null
    const pendingLoan = loans.loans.find(l => l.status === 'pending') ?? null
    const standing = loans.loading || loans.error ? null : loanStanding(loans.loans)

    // A source that failed contributes nothing rather than blanking the feed;
    // the feed only reports an error when there's nothing left to show.
    const activity = recentActivity(
        transactions.error ? [] : transactions.items,
        payments.error ? [] : payments.payments,
        loans.error ? [] : loans.loans,
    )
    const activityLoading = loans.loading || transactions.loading || payments.loading
    const activityError = loans.error && transactions.error && payments.error

    if (loading) {
        return <DashboardSkeleton />
    }

    if (error || !data) {
        return (
            <>
                <header className='dash-head'>
                    <div>
                        <p className='dash-eyebrow'>Overview</p>
                        <h1>Your position today</h1>
                    </div>
                </header>
                <section className='lending-card'>
                    <p className='lending-muted'>Couldn’t load your dashboard. Please try again later.</p>
                    <button type='button' className='lending-btn' onClick={refresh}>Retry</button>
                </section>
            </>
        )
    }

    return (
        <>
            <header className='dash-head'>
                <div>
                    <p className='dash-eyebrow'>Overview</p>
                    <h1>Your position today</h1>
                    <p>Everything you have lent, borrowed and owe, in one place.</p>
                </div>
                <div className='dash-head-meta'>
                    {standing && <span className={`dash-standing ${standing.cls}`}>{standing.label}</span>}
                    <span className='dash-date'>{today}</span>
                </div>
            </header>

            <SummaryTiles data={data} loan={loan} pendingLoan={pendingLoan} loansLoading={loans.loading} />

            <div className='dash-layout'>
                <aside className='dash-rail'>
                    <BalanceCard data={data} />
                    <EligibilityCard data={data} />
                    <TierLimitsCard data={data} />
                </aside>

                <div className='dash-main'>
                    <NextPaymentCard
                        loan={loan}
                        pendingLoan={pendingLoan}
                        loading={loans.loading}
                        error={loans.error}
                        repaid={payments.totals}
                        paymentCount={payments.total}
                    />
                    <RecentActivityCard rows={activity} loading={activityLoading} error={activityError} />
                    <PoolStrip data={data} />
                    <GuarantorCard onChanged={handleChanged} />
                </div>
            </div>
        </>
    )
}

function Dashboard() {
    const { user } = useSession()
    // Split before any data hook runs: restricted accounts never mount
    // Overview, so they never fire requests for features they can't use yet.
    const restricted = user?.role === 'Pending' || user?.role === 'Verifying'

    return (
        <main className='dash-page'>
            {restricted ? <VerifyPrompt verifying={user?.role === 'Verifying'} /> : <Overview />}
        </main>
    )
}

export default Dashboard
