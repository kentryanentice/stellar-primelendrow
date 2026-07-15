import { signTransaction, isConnected as freighterIsConnected } from '@stellar/freighter-api'

/**
 * Builds, signs (Freighter), and submits the vault contract's `lock` call —
 * the ONLY contract entry a user ever invokes. Release and seize are
 * admin-gated inside the contract itself, so nothing this file could be
 * tampered into doing can move funds out of the vault.
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

export type LockResult = { txHash: string } | { error: string }

export async function lockCollateralOnChain(opts: {
    contractId: string
    walletAddress: string
    loanId: string
    stroops: number
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
        )

        const built = new sdk.TransactionBuilder(account, {
            fee: sdk.BASE_FEE,
            networkPassphrase: NETWORK_PASSPHRASE,
        })
            .addOperation(operation)
            .setTimeout(180)
            .build()

        // Simulation attaches the Soroban footprint, auth entries, and the
        // real resource fee — signing the unprepared tx would just fail.
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
            return { error: 'The network rejected the lock transaction' }
        }

        // Poll until the ledger closes over it (~5s), bounded so a stalled
        // network returns a retriable message instead of hanging forever.
        for (let attempt = 0; attempt < 30; attempt++) {
            await sleep(2000)
            const result = await server.getTransaction(sendResponse.hash)
            if (result.status === 'SUCCESS') return { txHash: sendResponse.hash }
            if (result.status === 'FAILED') return { error: 'The lock transaction failed on-chain' }
        }
        return { error: 'Still waiting for the network — if your wallet shows the lock went through, press Confirm again' }
    } catch (err) {
        return { error: err instanceof Error ? err.message : 'Unable to lock collateral' }
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
