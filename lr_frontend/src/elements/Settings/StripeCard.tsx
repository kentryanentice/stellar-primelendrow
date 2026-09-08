import { useEffect } from 'react'
import { BadgeCheck, CircleAlert, CreditCard, Loader2, Unlink } from 'lucide-react'
import useStripeAccount, { stripeConnectResult } from '../../functions/Lending/useStripeAccount'
import { useToast } from '../../providers/useToast'

const formatConnected = (secs: number) =>
    new Date(secs * 1000).toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' })

/**
 * The Settings "Stripe" card: connect the account payouts get sent to, see
 * which one is linked, and unlink it.
 *
 * The mirror of PaypalCard, with one extra state it has to render honestly.
 * Stripe onboarding is a form the member fills in on stripe.com — identity
 * details, a bank account — and they can leave half-way through, or finish and
 * still be waiting on Stripe. Both of those look like "linked" in the database
 * and are not payable, so this card shows them as *unfinished* and offers to
 * resume rather than claiming success. Telling someone they're connected and
 * then refusing their withdrawal is the failure this avoids.
 */
export default function StripeCard() {
    const { account, loading, busy, connect, disconnect } = useStripeAccount()
    const toast = useToast()

    // The engine's return URL redirects back here with ?stripe=… — say how it
    // went, once, and clean the URL.
    useEffect(() => {
        const result = stripeConnectResult()
        if (!result) return
        if (result.ok) toast.success(result.message)
        else toast.error(result.message)
    }, [toast])

    return (
        <section className='settings-card settings-card-stripe'>
            <div className='settings-card-head'>
                <span className='settings-card-icon is-accent'><CreditCard /></span>
                <h2>Stripe</h2>
            </div>

            <p className='settings-muted'>
                Where your money is sent when you withdraw it. You’ll set it up on Stripe’s own site — we never
                ask you to type an account number, so there’s nothing to get wrong.
            </p>

            {loading ? (
                <p className='settings-muted'>Checking…</p>
            ) : !account?.stripe_ready ? (
                <p className='settings-muted'>
                    <CircleAlert /> Stripe payouts aren’t enabled on this deployment yet.
                </p>
            ) : account.connected ? (
                <>
                    <div className='settings-paypal-row'>
                        <div>
                            <b>{account.email_masked ?? 'Connected account'}</b>
                            <span className='settings-wallet-badge'><BadgeCheck /> Ready for payouts</span>
                        </div>
                        {account.connected_at !== null && (
                            <span className='settings-muted'>Connected {formatConnected(account.connected_at)}</span>
                        )}
                    </div>
                    <button
                        type='button'
                        className='settings-wallet-disconnect settings-paypal-disconnect'
                        disabled={busy}
                        onClick={() => void disconnect()}
                    >
                        {busy ? <Loader2 className='settings-wallet-spin' /> : <Unlink />}
                        Disconnect
                    </button>
                </>
            ) : account.onboarding ? (
                <>
                    {/* Started, not payable. The button resumes the same
                        account rather than starting a second one — the engine
                        reuses whatever it already created for this member. */}
                    <p className='settings-muted'>
                        <CircleAlert /> Stripe still needs a few details before it can pay you. Your setup is saved —
                        picking it back up takes a minute.
                    </p>
                    <button type='button' className='settings-btn-primary' disabled={busy} onClick={() => void connect()}>
                        {busy ? <Loader2 className='settings-wallet-spin' /> : <CreditCard />}
                        Finish setting up
                    </button>
                </>
            ) : (
                <button type='button' className='settings-btn-primary' disabled={busy} onClick={() => void connect()}>
                    {busy ? <Loader2 className='settings-wallet-spin' /> : <CreditCard />}
                    Connect Stripe
                </button>
            )}
        </section>
    )
}
