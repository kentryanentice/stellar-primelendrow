import {
    getNetworkDetails as freighterGetNetworkDetails,
    isConnected as freighterIsConnected,
    requestAccess as freighterRequestAccess,
    signMessage as freighterSignMessage,
    signTransaction as freighterSignTransaction,
} from '@stellar/freighter-api'
import { WalletConnectModule, WalletConnectTargetChain } from '@creit.tech/stellar-wallets-kit/modules/wallet-connect'
import { apiFetch } from '../apiFetch'
import { loadPublicConfig } from '../publicConfig'

const API = import.meta.env.VITE_API_URL ?? ''

// The network the app runs on, the same switch stellarLock and stellarAdmin
// read. WalletConnect has to be told explicitly: left to itself the kit asks
// the wallet for mainnet, so Freighter Mobile set to testnet refused to pair.
const IS_MAINNET = import.meta.env.VITE_STELLAR_NETWORK === 'public'
const WC_CHAIN = IS_MAINNET ? WalletConnectTargetChain.PUBLIC : WalletConnectTargetChain.TESTNET
const NETWORK_PASSPHRASE = IS_MAINNET
    ? 'Public Global Stellar Network ; September 2015'
    : 'Test SDF Network ; September 2015'

// ---- the wallet must be on the app's network ----
//
// A wallet left on another network still connects and still signs — the
// address is the same key everywhere — but whatever it signs is for a ledger
// the engine never reads: a lock that lands on Mainnet is real XLM moved
// somewhere this app can't see. So every connect and every signature checks
// the wallet's network first and refuses with a sentence that says what to
// switch, rather than failing later in a way nobody can read.

const APP_NETWORK = IS_MAINNET ? 'Mainnet' : 'Testnet'

const NETWORK_NAMES: Record<string, string> = {
    'Public Global Stellar Network ; September 2015': 'Mainnet',
    'Test SDF Network ; September 2015': 'Testnet',
    'Test SDF Future Network ; October 2022': 'Futurenet',
}

const wrongNetwork = (walletNetwork: string) =>
    `Your wallet is on ${walletNetwork}, but PrimeLendRow runs on ${APP_NETWORK}. `
    + `Switch your wallet to ${APP_NETWORK} and try again.`

/**
 * Null when the Freighter extension is on the app's network, otherwise what
 * to tell the member. Fails closed: a network it can't read is refused too.
 * Exported for the admin vault signer, which talks to the extension directly.
 */
export async function extensionNetworkError(): Promise<string | null> {
    const { networkPassphrase, error } = await freighterGetNetworkDetails()
    if (error || !networkPassphrase) {
        return 'Couldn’t tell which network your wallet is on — unlock Freighter and try again'
    }
    if (networkPassphrase === NETWORK_PASSPHRASE) return null
    return wrongNetwork(NETWORK_NAMES[networkPassphrase] ?? 'a different network')
}

/**
 * The same check for a WalletConnect wallet, which can't be asked its network
 * directly — but a session is approved per chain, and each account in it is
 * named `stellar:<chain>:<address>`, so the session says which network the
 * paired wallet signs for.
 */
function sessionNetworkError(wc: WalletConnectModule, address: string): string | null {
    const chains = wc.signClient.session
        .getAll()
        .flatMap(session => session.namespaces.stellar?.accounts ?? [])
        .filter(account => account.endsWith(`:${address}`))
        .map(account => account.slice(0, account.lastIndexOf(':')))
    if (chains.includes(WC_CHAIN)) return null
    if (chains.length === 0) return 'Your wallet’s session has ended — connect it again'
    return wrongNetwork(chains.includes(WalletConnectTargetChain.PUBLIC) ? 'Mainnet' : 'a different network')
}

// Browser extensions don't exist on phones, so a phone visiting this page has
// no way to satisfy the old "Freighter extension installed" check — this is
// what actually made mobile look unsupported. WalletConnect is the bridge:
// it pairs with Freighter Mobile (or any other WalletConnect-compatible
// Stellar wallet) via a QR code on desktop or a deep link on the phone
// itself, so the same "Connect" button works either way.
//
// Get a free project id at https://cloud.reown.com (Reown Cloud, formerly
// WalletConnect Cloud) and set WALLETCONNECT_PROJECT_ID in lr_engine's .env;
// the app reads it from the engine's GET /config (functions/publicConfig).
// Until it's set, connectFreighter() falls back to extension-only, exactly
// like before. Lock the project to your domains in Reown Cloud — the id is
// public by nature, and the domain allowlist is what stops other sites using it.

/** How many leading characters of a wallet address to show before the ellipsis. */
export const ADDRESS_DISPLAY_LEN = 20

/** Full address is still what's stored and submitted — this only shortens what's shown. */
export const truncateAddress = (address: string) =>
    address.length > ADDRESS_DISPLAY_LEN ? `${address.slice(0, ADDRESS_DISPLAY_LEN)}…` : address

let wcModule: WalletConnectModule | null = null

/** Created once (module init kicks off a real network call to the WalletConnect
 *  relay), reused for every connect attempt rather than re-pairing from scratch.
 *  Null when the engine has no project id configured. */
async function getWalletConnectModule(): Promise<WalletConnectModule | null> {
    const { walletconnect_project_id: projectId } = await loadPublicConfig()
    if (!projectId) return null
    if (!wcModule) {
        wcModule = new WalletConnectModule({
            projectId,
            allowedChains: [WC_CHAIN],
            metadata: {
                name: 'PrimeLendRow',
                description: 'PrimeLendRow identity verification',
                url: window.location.origin,
                icons: ['https://primelendrow.com/pictures/primelendrow.webp'],
            },
        })
    }
    return wcModule
}

// Module load (SignClient.init(...)) is async and un-awaited inside the kit's
// own constructor — polls isAvailable() rather than assuming it's ready the
// instant a user clicks Connect, which they can do before that finishes.
async function waitUntilReady(module: WalletConnectModule, timeoutMs = 8000): Promise<void> {
    const start = Date.now()
    while (!(await module.isAvailable())) {
        if (Date.now() - start > timeoutMs) {
            throw new Error('WalletConnect is taking too long to start — check your connection and try again')
        }
        await new Promise(resolve => setTimeout(resolve, 100))
    }
}

export type ConnectResult = { address: string } | { error: string }

export const CONNECT_CANCELLED = 'Wallet connection cancelled'

/**
 * Rejects once the WalletConnect window is closed without a wallet having
 * paired. The kit's `getAddress` waits on the pairing approval alone, and
 * closing the window does not cancel that wait — so without this, dismissing
 * the QR/deep-link window left "Connecting…" on screen indefinitely.
 *
 * A successful pairing closes the window too, a moment before `getAddress`
 * returns; the short grace period lets that result win the race instead of
 * being mistaken for a cancel.
 */
function cancelledWhenWindowCloses(wc: WalletConnectModule) {
    let timer: ReturnType<typeof setTimeout> | undefined
    let unsubscribe = () => {}
    const promise = new Promise<never>((_, reject) => {
        let wasOpen = false
        unsubscribe = wc.modal.subscribeState(state => {
            if (state.open) {
                wasOpen = true
                clearTimeout(timer)
            } else if (wasOpen) {
                timer = setTimeout(() => reject(new Error(CONNECT_CANCELLED)), 500)
            }
        })
    })
    return {
        promise,
        stop: () => {
            clearTimeout(timer)
            unsubscribe()
        },
    }
}

/**
 * Connects to Freighter, preferring the browser extension when it's
 * installed and falling back to WalletConnect — which is how Freighter
 * Mobile (and other WalletConnect-compatible Stellar wallets) get reached —
 * when it isn't.
 */
export async function connectFreighter(): Promise<ConnectResult> {
    const { isConnected: hasExtension } = await freighterIsConnected()
    if (hasExtension) {
        const { address, error } = await freighterRequestAccess()
        if (error || !address) return { error: error?.message ?? 'Unable to connect wallet' }
        const networkError = await extensionNetworkError()
        if (networkError) return { error: networkError }
        return { address }
    }

    const wc = await getWalletConnectModule()
    if (!wc) {
        return {
            error: 'Freighter extension not detected. Install the browser extension, or ask an admin to enable WalletConnect for mobile.',
        }
    }
    const closed = cancelledWhenWindowCloses(wc)
    try {
        await waitUntilReady(wc)
        const pairing = wc.getAddress()
        // Once the window is closed nobody awaits the pairing any more; its
        // eventual expiry must not surface as an unhandled rejection.
        pairing.catch(() => {})
        const { address } = await Promise.race([pairing, closed.promise])
        const networkError = sessionNetworkError(wc, address)
        if (networkError) {
            // Not kept: a session for the wrong network would only fail
            // again at the first signature.
            await wc.disconnect().catch(() => {})
            return { error: networkError }
        }
        return { address }
    } catch (e) {
        return { error: e instanceof Error ? e.message : 'Unable to connect via WalletConnect' }
    } finally {
        closed.stop()
    }
}

/** No-op when the active connection was the browser extension — nothing to tear down there. */
export async function disconnectFreighter(): Promise<void> {
    if (!wcModule) return
    try {
        await wcModule.disconnect()
    } catch {
        // no active WalletConnect session to close
    }
}

export type SignResult = { signature: string } | { error: string }

/**
 * Signs an arbitrary message (SEP-0053) with the given address, proving
 * control of its private key — used to prove ownership of a wallet before
 * the backend connects it to an account (see functions/Wallet/useWallets.ts).
 * Mirrors connectFreighter's extension-then-WalletConnect precedence rather
 * than tracking which path the address came from.
 *
 * Deliberately does not call disconnectFreighter(): tearing down the live
 * wallet session is a separate concern from marking a database row
 * "disconnected".
 */
export async function signChallenge(message: string, address: string): Promise<SignResult> {
    const { isConnected: hasExtension } = await freighterIsConnected()
    if (hasExtension) {
        const networkError = await extensionNetworkError()
        if (networkError) return { error: networkError }
        const { signedMessage, error } = await freighterSignMessage(message, { address })
        if (error || !signedMessage) return { error: error?.message ?? 'Unable to sign verification message' }
        // Older extension builds (protocol v3) return the raw signature as a
        // Buffer instead of an already-base64 string (v4) — normalize to the
        // base64 form the backend expects either way.
        const signature = typeof signedMessage === 'string' ? signedMessage : signedMessage.toString('base64')
        return { signature }
    }

    const wc = await getWalletConnectModule()
    if (!wc) {
        return { error: 'Freighter extension not detected. Install the browser extension, or ask an admin to enable WalletConnect for mobile.' }
    }
    try {
        await waitUntilReady(wc)
        const networkError = sessionNetworkError(wc, address)
        if (networkError) return { error: networkError }
        const { signedMessage } = await wc.signMessage(message, { address, networkPassphrase: NETWORK_PASSPHRASE })
        return { signature: signedMessage }
    } catch (e) {
        return { error: e instanceof Error ? e.message : 'Unable to sign verification message' }
    }
}

export type ProofResult = { nonce: string; signature: string } | { error: string }

/**
 * Proves the connected wallet controls `address`: asks the engine for a
 * one-time challenge (POST /wallets/challenge) and has the wallet sign it.
 * Wherever the proof is needed — connecting a wallet from Settings, or
 * submitting KYC — the engine uses the pair up exactly once, so a fresh one
 * is made for every attempt rather than kept.
 */
export async function proveWallet(address: string, csrfToken: string | null): Promise<ProofResult> {
    let challenge: { nonce: string; message: string }
    try {
        const res = await apiFetch(`${API}/wallets/challenge`, {
            method: 'POST',
            credentials: 'include',
            headers: {
                'Content-Type': 'application/json',
                ...(csrfToken ? { 'x-csrf-token': csrfToken } : {}),
            },
        })
        if (!res.ok) return { error: await res.text() || 'Unable to start wallet verification' }
        challenge = await res.json() as { nonce: string; message: string }
    } catch {
        return { error: 'Unable to start wallet verification' }
    }

    const signed = await signChallenge(challenge.message, address)
    if ('error' in signed) return signed
    return { nonce: challenge.nonce, signature: signed.signature }
}

export type SignTxResult = { signedTxXdr: string } | { error: string }

/**
 * Signs a prepared transaction (base64 XDR) with the given address — the
 * browser extension when it's installed, otherwise WalletConnect, which is how
 * the Freighter Mobile app is asked to sign on a phone. Same precedence as
 * connectFreighter and signChallenge, so whichever way a member connected
 * their wallet is the way it signs.
 */
export async function signTransactionXdr(
    xdr: string,
    address: string,
    networkPassphrase: string,
): Promise<SignTxResult> {
    // A transaction built for another network is a caller's bug, never a
    // member's choice — refused before any wallet is asked.
    if (networkPassphrase !== NETWORK_PASSPHRASE) {
        return { error: `That transaction was built for ${NETWORK_NAMES[networkPassphrase] ?? 'a different network'}, not ${APP_NETWORK}` }
    }
    const { isConnected: hasExtension } = await freighterIsConnected()
    if (hasExtension) {
        const networkError = await extensionNetworkError()
        if (networkError) return { error: networkError }
        const signed = await freighterSignTransaction(xdr, { networkPassphrase, address })
        if (signed.error || !signed.signedTxXdr) return { error: signed.error?.message ?? 'Signing was cancelled' }
        return { signedTxXdr: signed.signedTxXdr }
    }

    const wc = await getWalletConnectModule()
    if (!wc) {
        return { error: 'Freighter extension not detected. Install the browser extension, or ask an admin to enable WalletConnect for mobile.' }
    }
    try {
        await waitUntilReady(wc)
        const networkError = sessionNetworkError(wc, address)
        if (networkError) return { error: networkError }
        const { signedTxXdr } = await wc.signTransaction(xdr, { networkPassphrase, address })
        if (!signedTxXdr) return { error: 'Signing was cancelled' }
        return { signedTxXdr }
    } catch (e) {
        return { error: e instanceof Error ? e.message : 'Signing was cancelled' }
    }
}
