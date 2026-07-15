import { useState } from 'react'
import { Wallet as WalletIcon, ShieldCheck, Unlink, Plus, Loader2 } from 'lucide-react'
import useWallets from '../../functions/Wallet/useWallets'
import { truncateAddress } from '../../functions/Wallet/wallet'

/** Mirrors lr_engine::api::wallets::shared::MAX_WALLETS_PER_USER. */
const MAX_WALLETS = 5

// connected_at is unix seconds (Utc::now().timestamp() server-side), same
// convention as Settings.tsx's own formatDate/formatMemberSince.
const formatConnected = (secs: number) =>
    new Date(secs * 1000).toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' })

/**
 * The Settings "Wallets" card: shows every wallet on file (the KYC-reviewed
 * one included, badged), lets the caller connect another via a real
 * signature challenge, and disconnect any of them. Lazy-loaded from
 * Settings.tsx — the wallet-connect SDKs this pulls in via useWallets/wallet
 * shouldn't add weight to every Settings visit.
 */
export default function WalletsCard() {
    const { wallets, loading, error, connecting, disconnectingId, connectNewWallet, disconnectWallet } = useWallets()
    const [label, setLabel] = useState('')
    const [confirmingId, setConfirmingId] = useState<string | null>(null)

    const active = wallets.filter(w => w.status === 'active')
    const disconnected = wallets.filter(w => w.status !== 'active')
    const atLimit = active.length >= MAX_WALLETS

    const handleConnect = async () => {
        await connectNewWallet(label)
        setLabel('')
    }

    const handleDisconnect = async (id: string) => {
        setConfirmingId(null)
        await disconnectWallet(id)
    }

    return (
        <section className='settings-card settings-card-wallets'>
            <div className='settings-card-head'>
                <span className='settings-card-icon is-accent'><WalletIcon /></span>
                <h2>Wallets</h2>
                <span className='settings-pill is-muted'>{active.length} connected</span>
            </div>

            {loading ? (
                <p className='settings-muted'>Loading wallets…</p>
            ) : error ? (
                <p className='settings-muted'>Couldn’t load your wallets. Please try again later.</p>
            ) : (
                <>
                    {active.length === 0 ? (
                        <p className='settings-muted'>No wallets connected yet.</p>
                    ) : (
                        <ul className='settings-wallet-list'>
                            {active.map(w => (
                                <li key={w.id} className='settings-wallet-row'>
                                    <div className='settings-wallet-info'>
                                        <div className='settings-wallet-info-top'>
                                            <span className='settings-wallet-address' title={w.address}>{truncateAddress(w.address)}</span>
                                        </div>
                                        <div className='settings-wallet-meta-row'>
                                            {w.source === 'kyc_verified' && (
                                                <span className='settings-wallet-badge'><ShieldCheck /> KYC verified</span>
                                            )}
                                            <span className='settings-wallet-meta'>
                                                {w.label ? `${w.label} · ` : ''}Connected {formatConnected(w.connected_at)}
                                            </span>
                                        </div>
                                    </div>
                                    {confirmingId === w.id ? (
                                        <div className='settings-wallet-confirm'>
                                            <span>Disconnect?</span>
                                            <button type='button' onClick={() => handleDisconnect(w.id)} disabled={disconnectingId === w.id}>
                                                Yes
                                            </button>
                                            <button type='button' onClick={() => setConfirmingId(null)}>Cancel</button>
                                        </div>
                                    ) : (
                                        <button
                                            type='button'
                                            className='settings-wallet-disconnect'
                                            onClick={() => setConfirmingId(w.id)}
                                            disabled={disconnectingId !== null}
                                        >
                                            <Unlink /> Disconnect
                                        </button>
                                    )}
                                </li>
                            ))}
                        </ul>
                    )}

                    <div className='settings-wallet-add'>
                        <input
                            type='text'
                            className='settings-wallet-label-input'
                            placeholder='Label (optional)'
                            value={label}
                            onChange={e => setLabel(e.target.value)}
                            maxLength={50}
                            disabled={connecting || atLimit}
                        />
                        <button type='button' className='settings-btn-primary' onClick={handleConnect} disabled={connecting || atLimit}>
                            {connecting ? <Loader2 className='settings-wallet-spin' /> : <Plus />}
                            {connecting ? 'Connecting…' : 'Add wallet'}
                        </button>
                    </div>
                    {atLimit && (
                        <p className='settings-wallet-limit'>Wallet limit reached ({MAX_WALLETS}) — disconnect one to add another.</p>
                    )}

                    {disconnected.length > 0 && (
                        <details className='settings-wallet-history'>
                            <summary>Previously connected ({disconnected.length})</summary>
                            <ul className='settings-wallet-list is-history'>
                                {disconnected.map(w => (
                                    <li key={w.id} className='settings-wallet-row is-disconnected'>
                                        <div className='settings-wallet-info'>
                                            <div className='settings-wallet-info-top'>
                                                <span className='settings-wallet-address' title={w.address}>{truncateAddress(w.address)}</span>
                                            </div>
                                            <div className='settings-wallet-meta-row'>
                                                {w.source === 'kyc_verified' && (
                                                    <span className='settings-wallet-badge'><ShieldCheck /> KYC verified</span>
                                                )}
                                                <span className='settings-wallet-meta'>
                                                    {w.label ? `${w.label} · ` : ''}Disconnected {w.disconnected_at ? formatConnected(w.disconnected_at) : ''}
                                                </span>
                                            </div>
                                        </div>
                                    </li>
                                ))}
                            </ul>
                        </details>
                    )}
                </>
            )}
        </section>
    )
}
