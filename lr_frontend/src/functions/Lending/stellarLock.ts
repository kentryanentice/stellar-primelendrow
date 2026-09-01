import { signTransaction, isConnected as freighterIsConnected } from '@stellar/freighter-api'
import type { PinnedQuote } from './types'

/**
 * Builds, signs (Freighter), and submits the vault contract's `lock` call —
 * the ONLY contract entry a user ever invokes. Recording an outcome,
 * releasing and seizing are admin-gated inside the contract itself, so
 * nothing this file could be tampered into doing can move funds out of the
 * vault.
 *
 * The call carries the engine's pinned quote (the peso rate and the two legs
 * behind it) and the principal it covers. None of that is taken on trust
 * on-chain: the vault measures the dollar leg against a public SEP-40 feed,
 * refuses a stale feed or an out-of-band number, refuses a peso rate its own
 * legs don't support, and refuses coins that don't cover the ratio. A client
 * that lies here gets a failed transaction, not a cheap loan.
 *
 * The heavy @stellar/stellar-sdk is imported dynamically so visitors who
 * never touch XLM collateral don't download it.
 */

const RPC_URL = (import.meta.env.VITE_SOROBAN_RPC_URL as string | undefined) || 'https://soroban-testnet.stellar.org'
const NETWORK_PASSPHRASE = import.meta.env.VITE_STELLAR_NETWORK === 'public'
    ? 'Public Global Stellar Network ; September 2015'
    : 'Test SDF Network ; September 2015'

/** Loan UUID -> its 16 raw bytes, the BytesN<16> key the contract stores under. */
function uuidTo16Bytes(uuid: string): Uint8Array {
    const hex = uuid.replace(/-/g, '')
    const bytes = new Uint8Array(16)
    for (let i = 0; i < 16; i++) {
        bytes[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16)
    }
    return bytes
}

const sleep = (ms: number) => new Promise(resolve => setTimeout(resolve, ms))

/**
 * The vault's refusals, in the borrower's language. These are the states the
 * SOW asks to be demonstrable — a stale feed and an out-of-band quote are
 * supposed to happen and to be legible when they do, not to surface as a bare
 * "transaction failed". Codes are `Error` in lr_contracts/collateral_vault.
 */
const CONTRACT_REFUSALS: Record<number, string> = {
    3: 'The amount to lock must be more than zero',
    4: 'This loan already has collateral locked against it',
    5: 'The vault has no collateral recorded for this loan',
    6: 'The price on your application no longer holds together — start a new application to get a fresh quote',
    7: 'The public price feed has no XLM price right now, so the vault will not lock any collateral. Try again shortly',
    8: 'The public price feed’s latest price is too old for the vault to act on. Try again in a few minutes',
    9: 'XLM has moved too far from the price your application was quoted at — start a new application to lock at the current rate',
    10: 'The locked amount no longer covers the required collateral — start a new application to get a fresh quote',
}

/** Pull a contract error code out of whatever the RPC or SDK threw. */
function refusalFrom(err: unknown): string | null {
    const text = err instanceof Error ? err.message : String(err ?? '')
    const code = /Error\(Contract,\s*#(\d+)\)/.exec(text)
    return code ? CONTRACT_REFUSALS[Number(code[1])] ?? null : null
}

export type CollateralQuote = {
    php_per_xlm_centavos: number
    usd_per_xlm_e8: number
    php_per_usd_centavos: number
}

/**
 * The pinned quote in the contract's shape, or null when the position predates
 * the on-chain price check and has no dollar leg recorded — those can't be
 * locked and need a fresh application.
 */
export function quoteFromPinned(pinned: PinnedQuote): CollateralQuote | null {
    if (
        pinned.priced_centavos_per_xlm === null ||
        pinned.priced_usd_per_xlm_e8 === null ||
        pinned.priced_usd_php_centavos === null
    ) return null
    return {
        php_per_xlm_centavos: pinned.priced_centavos_per_xlm,
        usd_per_xlm_e8: pinned.priced_usd_per_xlm_e8,
        php_per_usd_centavos: pinned.priced_usd_php_centavos,
    }
}

export type LockResult = { txHash: string } | { error: string }

export async function lockCollateralOnChain(opts: {
    contractId: string
    walletAddress: string
    loanId: string
    stroops: number
    /** Whole centavos of principal this collateral stands behind. */
    principalCentavos: number
    quote: CollateralQuote
}): Promise<LockResult> {
    const { isConnected } = await freighterIsConnected()
    if (!isConnected) {
        return { error: 'Locking collateral needs the Freighter browser extension' }
    }

    try {
        const sdk = await import('@stellar/stellar-sdk')
        const server = new sdk.rpc.Server(RPC_URL)

        const account = await server.getAccount(opts.walletAddress)
        const contract = new sdk.Contract(opts.contractId)
        const operation = contract.call(
            'lock',
            new sdk.Address(opts.walletAddress).toScVal(),
            sdk.nativeToScVal(uuidTo16Bytes(opts.loanId), { type: 'bytes' }),
            sdk.nativeToScVal(BigInt(opts.stroops), { type: 'i128' }),
            sdk.nativeToScVal(BigInt(opts.principalCentavos), { type: 'i128' }),
            // A struct is an SCMap keyed by symbols; nativeToScVal sorts the
            // keys, and the type spec is what stops them going over as
            // strings, which the contract would not recognise.
            sdk.nativeToScVal(
                {
                    php_per_xlm_centavos: BigInt(opts.quote.php_per_xlm_centavos),
                    php_per_usd_centavos: BigInt(opts.quote.php_per_usd_centavos),
                    usd_per_xlm_e8: BigInt(opts.quote.usd_per_xlm_e8),
                },
                {
                    type: {
                        php_per_xlm_centavos: ['symbol', 'i128'],
                        php_per_usd_centavos: ['symbol', 'i128'],
                        usd_per_xlm_e8: ['symbol', 'i128'],
                    },
                },
            ),
        )

        const built = new sdk.TransactionBuilder(account, {
            fee: sdk.BASE_FEE,
            networkPassphrase: NETWORK_PASSPHRASE,
        })
            .addOperation(operation)
            .setTimeout(180)
            .build()

        // Simulation attaches the Soroban footprint, auth entries, and the
        // real resource fee — signing the unprepared tx would just fail. It is
        // also where the vault's price and ratio checks run, so a refusal
        // usually lands here, before the borrower is asked to sign anything.
        const prepared = await server.prepareTransaction(built)

        const signed = await signTransaction(prepared.toXDR(), {
            networkPassphrase: NETWORK_PASSPHRASE,
            address: opts.walletAddress,
        })
        if (signed.error || !signed.signedTxXdr) {
            return { error: signed.error?.message ?? 'Signing was cancelled' }
        }

        const sendResponse = await server.sendTransaction(
            sdk.TransactionBuilder.fromXDR(signed.signedTxXdr, NETWORK_PASSPHRASE),
        )
        if (sendResponse.status === 'ERROR') {
            return { error: refusalFrom(sendResponse.errorResult) ?? 'The network rejected the lock transaction' }
        }

        // Poll until the ledger closes over it (~5s), bounded so a stalled
        // network returns a retriable message instead of hanging forever.
        for (let attempt = 0; attempt < 30; attempt++) {
            await sleep(2000)
            const result = await server.getTransaction(sendResponse.hash)
            if (result.status === 'SUCCESS') return { txHash: sendResponse.hash }
            if (result.status === 'FAILED') {
                return { error: refusalFrom(result.resultXdr) ?? 'The lock transaction failed on-chain' }
            }
        }
        return { error: 'Still waiting for the network — if your wallet shows the lock went through, press Confirm again' }
    } catch (err) {
        return {
            error: refusalFrom(err)
                ?? (err instanceof Error ? err.message : 'Unable to lock collateral'),
        }
    }
}

const API = import.meta.env.VITE_API_URL ?? ''

/**
 * The full continuation for a pending XLM loan: lock on-chain, then hand the
 * tx hash to the engine, which verifies it against Horizon and disburses.
 * Used by both the borrow wizard (fresh application) and the loans list
 * (resuming after a reload or an interrupted lock).
 */
export async function lockAndConfirmCollateral(opts: {
    contractId: string
    walletAddress: string
    loanId: string
    stroops: number
    principalCentavos: number
    quote: CollateralQuote
    csrfToken: string | null
}): Promise<{ message: string } | { error: string }> {
    const lock = await lockCollateralOnChain(opts)
    if ('error' in lock) return lock

    try {
        const res = await fetch(`${API}/collateral/confirm`, {
            method: 'POST',
            credentials: 'include',
            headers: {
                'Content-Type': 'application/json',
                ...(opts.csrfToken ? { 'x-csrf-token': opts.csrfToken } : {}),
            },
            body: JSON.stringify({ loan_id: opts.loanId, tx_hash: lock.txHash }),
        })
        if (!res.ok) throw new Error(await res.text() || 'Unable to confirm the collateral')
        const data = await res.json() as { message: string }
        return { message: data.message }
    } catch (err) {
        return { error: err instanceof Error ? err.message : 'Unable to confirm the collateral' }
    }
}
