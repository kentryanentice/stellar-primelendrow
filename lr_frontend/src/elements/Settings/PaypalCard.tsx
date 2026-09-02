import { useEffect } from 'react'
import { BadgeCheck, CircleAlert, Loader2, Unlink, Wallet } from 'lucide-react'
import usePaypalAccount, { paypalRedirectResult } from '../../functions/Lending/usePaypalAccount'
import { useToast } from '../../providers/useToast'

const formatConnected = (secs: number) =>
    new Date(secs * 1000).toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' })

/**
 * The Settings "PayPal" card: connect the account loan proceeds get paid to,
 * see which one is linked, and unlink it.
 *
 * Connecting leaves the app — the member signs in on paypal.com and comes
 * back through the engine's callback. That is the point of doing it this way
 * rather than asking for an email: the destination is an account PayPal
 * itself confirmed, so there is no address anyone can mistype.
 */
export default function PaypalCard() {
    const { account, loading, busy, connect, disconnect } = usePaypalAccount()
    const toast = useToast()

    // The engine's callback redirects back here with ?paypal=… — say how it
    // went, once, and clean the URL.
    useEffect(() => {
        const result = paypalRedirectResult()
        if (!result) return
        if (result.ok) toast.success(result.message)
        else toast.error(result.message)
    }, [toast])

    return (
        <section className='settings-card settings-card-paypal'>
            <div className='settings-card-head'>
                <span className='settings-card-icon is-accent'><Wallet /></span>
                <h2>PayPal</h2>
            </div>

            <p className='settings-muted'>
                Where your loan is sent when you withdraw it. You’ll sign in on PayPal’s own site — we never
                ask you to type an address, so there’s nothing to get wrong.
            </p>

            {loading ? (
                <p className='settings-muted'>Checking…</p>
            ) : !account?.paypal_ready ? (
                <p className='settings-muted'>
                    <CircleAlert /> PayPal payouts aren’t enabled on this deployment yet.
                </p>
            ) : account.connected ? (
                <>
                    <div className='settings-paypal-row'>
                        <div>
                            <b>{account.email_masked}</b>
                            {account.verified && (
                                <span className='settings-wallet-badge'><BadgeCheck /> Verified</span>
                            )}
                        </div>
                        {account.connected_at !== null && (
                            <span className='settings-muted'>Connected {formatConnected(account.connected_at)}</span>
                        )}
                    </div>
                    {!account.verified && (
                        <p className='settings-muted'>
                            <CircleAlert /> PayPal hasn’t verified this account. It can still receive money, but
                            PayPal may hold it until the account is verified.
                        </p>
                    )}
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
            ) : (
                <button type='button' className='settings-btn-primary' disabled={busy} onClick={() => void connect()}>
                    {busy ? <Loader2 className='settings-wallet-spin' /> : <Wallet />}
                    Connect PayPal
                </button>
            )}
        </section>
    )
}
