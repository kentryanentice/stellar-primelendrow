import {
    isConnected as freighterIsConnected,
    requestAccess as freighterRequestAccess,
    signMessage as freighterSignMessage,
} from '@stellar/freighter-api'
import { WalletConnectModule } from '@creit.tech/stellar-wallets-kit/modules/wallet-connect'

// Browser extensions don't exist on phones, so a phone visiting this page has
// no way to satisfy the old "Freighter extension installed" check — this is
// what actually made mobile look unsupported. WalletConnect is the bridge:
// it pairs with Freighter Mobile (or any other WalletConnect-compatible
// Stellar wallet) via a QR code on desktop or a deep link on the phone
// itself, so the same "Connect" button works either way.
//
// Get a free project id at https://cloud.reown.com (Reown Cloud, formerly
// WalletConnect Cloud) and set VITE_WALLETCONNECT_PROJECT_ID in .env — until
// then this stays undefined and connectFreighter() falls back to
// extension-only, exactly like before.
const PROJECT_ID = import.meta.env.VITE_WALLETCONNECT_PROJECT_ID as string | undefined

/** How many leading characters of a wallet address to show before the ellipsis. */
export const ADDRESS_DISPLAY_LEN = 20

/** Full address is still what's stored and submitted — this only shortens what's shown. */
export const truncateAddress = (address: string) =>
    address.length > ADDRESS_DISPLAY_LEN ? `${address.slice(0, ADDRESS_DISPLAY_LEN)}…` : address

let wcModule: WalletConnectModule | null = null

/** Created once (module init kicks off a real network call to the WalletConnect
 *  relay), reused for every connect attempt rather than re-pairing from scratch. */
function getWalletConnectModule(): WalletConnectModule | null {
    if (!PROJECT_ID) return null
    if (!wcModule) {
        wcModule = new WalletConnectModule({
            projectId: PROJECT_ID,
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
        return { address }
    }

    const wc = getWalletConnectModule()
    if (!wc) {
        return {
            error: 'Freighter extension not detected. Install the browser extension, or ask an admin to enable WalletConnect for mobile.',
        }
    }
    try {
        await waitUntilReady(wc)
        const { address } = await wc.getAddress()
        return { address }
    } catch (e) {
        return { error: e instanceof Error ? e.message : 'Unable to connect via WalletConnect' }
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
        const { signedMessage, error } = await freighterSignMessage(message, { address })
        if (error || !signedMessage) return { error: error?.message ?? 'Unable to sign verification message' }
        // Older extension builds (protocol v3) return the raw signature as a
        // Buffer instead of an already-base64 string (v4) — normalize to the
        // base64 form the backend expects either way.
        const signature = typeof signedMessage === 'string' ? signedMessage : signedMessage.toString('base64')
        return { signature }
    }

    const wc = getWalletConnectModule()
    if (!wc) {
        return { error: 'Freighter extension not detected. Install the browser extension, or ask an admin to enable WalletConnect for mobile.' }
    }
    try {
        await waitUntilReady(wc)
        const { signedMessage } = await wc.signMessage(message, { address })
        return { signature: signedMessage }
    } catch (e) {
        return { error: e instanceof Error ? e.message : 'Unable to sign verification message' }
    }
}
