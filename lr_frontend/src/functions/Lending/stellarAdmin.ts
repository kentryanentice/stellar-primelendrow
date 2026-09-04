import { signTransaction, requestAccess, isConnected as freighterIsConnected } from '@stellar/freighter-api'

/**
 * The four admin-gated vault movements, signed in the operator's own wallet.
 *
 * This is the deliberate mirror of stellarLock.ts. There, the BORROWER signs
 * the one entry point a user is allowed to call; here the ADMIN signs the four
 * they are. The engine never signs either: the key that can move coins out of
 * the vault is the one thing a compromised server must not be able to use, so
 * it lives in a wallet extension and the backend only ever verifies what the
 * chain did afterwards.
 *
 * Nothing here decides anything. The loan, the destination and the price all
 * come from `/lending/admin/actions/prepare` — the engine pins the seizure
 * quote and reads the treasury out of its own environment, so an operator can
 * choose WHETHER to sign, never what they are signing for.
 *
 * Ordering is the contract's business, not this file's: `release` is refused
 * unless the loan was recorded repaid, `seize` unless a default was recorded.
 * A movement signed out of order fails on-chain, which is the intended
 * behaviour and why the engine can trust the pair.
 */

const RPC_URL = (import.meta.env.VITE_SOROBAN_RPC_URL as string | undefined) || 'https://soroban-testnet.stellar.org'
const NETWORK_PASSPHRASE = import.meta.env.VITE_STELLAR_NETWORK === 'public'
    ? 'Public Global Stellar Network ; September 2015'
    : 'Test SDF Network ; September 2015'

export type VaultAction = 'mark_repaid' | 'release' | 'mark_defaulted' | 'seize'

export type SeizureQuote = {
    php_per_xlm_centavos: number
    usd_per_xlm_e8: number
    php_per_usd_centavos: number
}

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
 * The vault's refusals an operator can actually hit, in their language. The
 * ordering errors are the interesting ones: they mean the contract is doing
 * its job — a seizure with no recorded default behind it is exactly what the
 * state machine exists to refuse.
 */
const CONTRACT_REFUSALS: Record<number, string> = {
    2: 'The vault contract hasn’t been initialized on this network',
    5: 'The vault has no collateral recorded for this loan — it may already have been released or seized',
    6: 'The seizure price doesn’t hold together against its own legs — prepare the movement again',
    7: 'The public price feed has no XLM price right now, so the vault will not act on a seizure. Try again shortly',
    8: 'The public price feed’s latest price is too old for the vault to act on. Prepare the movement again for a fresh quote',
    9: 'The seizure price has moved too far from the public feed — prepare the movement again',
    11: 'The loan is still open on-chain: record the outcome with mark_repaid or mark_defaulted first, then move the coins',
    12: 'No default is recorded against this position on-chain, so the vault refuses to seize it',
    13: 'This position already has an outcome recorded against it — positions settle once',
}

function refusalFrom(err: unknown): string | null {
    const text = err instanceof Error ? err.message : String(err ?? '')
    const code = /Error\(Contract,\s*#(\d+)\)/.exec(text)
    return code ? CONTRACT_REFUSALS[Number(code[1])] ?? null : null
}

export type MovementResult = { txHash: string } | { error: string }

/** "GCYR…JL4J" — enough to tell two accounts apart in a message. */
const shortAddr = (a: string) => `${a.slice(0, 4)}…${a.slice(-4)}`

/**
 * Who the vault will accept a movement from, read from the contract itself.
 *
 * Worth a round trip before asking anyone to sign. `require_auth` failing is
 * not one of the contract's numbered errors — it's a host trap, so it arrives
 * as an opaque "failed account authentication" with no hint that the fix is
 * "switch accounts in Freighter". Since the admin is fixed at initialize and
 * there is no rotation, comparing it against the active account turns the most
 * likely operator mistake into a sentence that says what to do.
 *
 * Returns null if the read itself fails: an unreachable RPC shouldn't block a
 * signature that might well have worked.
 */
async function vaultAdmin(
    sdk: typeof import('@stellar/stellar-sdk'),
    server: InstanceType<typeof import('@stellar/stellar-sdk').rpc.Server>,
    contract: InstanceType<typeof import('@stellar/stellar-sdk').Contract>,
    sourceAddress: string,
): Promise<string | null> {
    try {
        const account = await server.getAccount(sourceAddress)
        const probe = new sdk.TransactionBuilder(account, {
            fee: sdk.BASE_FEE,
            networkPassphrase: NETWORK_PASSPHRASE,
        })
            .addOperation(contract.call('get_admin'))
            .setTimeout(30)
            .build()

        // Read-only: simulated, never submitted, never signed.
        const sim = await server.simulateTransaction(probe)
        if (!sdk.rpc.Api.isSimulationSuccess(sim) || !sim.result?.retval) return null
        const admin = sdk.scValToNative(sim.result.retval)
        return typeof admin === 'string' ? admin : null
    } catch {
        return null
    }
}

export async function submitVaultMovement(opts: {
    contractId: string
    action: VaultAction
    loanId: string
    /** Seizures only: where the coins go, and what they're valued at. Both
     *  come from the engine — see the module note. */
    treasury?: string | null
    quote?: SeizureQuote | null
}): Promise<MovementResult> {
    const { isConnected } = await freighterIsConnected()
    if (!isConnected) {
        return { error: 'Signing a vault movement needs the Freighter browser extension' }
    }
    if (opts.action === 'seize' && (!opts.treasury || !opts.quote)) {
        return { error: 'That seizure hasn’t been prepared — prepare it first so the engine can pin the price' }
    }

    try {
        const access = await requestAccess()
        if (access.error || !access.address) {
            return { error: access.error?.message ?? 'Freighter did not share an address' }
        }
        const signer = access.address

        const sdk = await import('@stellar/stellar-sdk')
        const server = new sdk.rpc.Server(RPC_URL)

        const contract = new sdk.Contract(opts.contractId)

        // Checked before anything is built, so the operator is told to switch
        // accounts instead of watching a transaction die on-chain.
        const admin = await vaultAdmin(sdk, server, contract, signer)
        if (admin && admin !== signer) {
            return {
                error: `This vault only accepts movements from its admin ${shortAddr(admin)}, `
                    + `and Freighter is currently on ${shortAddr(signer)}. `
                    + `Switch accounts in Freighter and try again.`,
            }
        }

        const account = await server.getAccount(signer)
        const loanKey = sdk.nativeToScVal(uuidTo16Bytes(opts.loanId), { type: 'bytes' })

        const operation = opts.action === 'seize'
            ? contract.call(
                'seize',
                loanKey,
                new sdk.Address(opts.treasury as string).toScVal(),
                // Same struct shape the lock sends: an SCMap keyed by symbols,
                // with the type spec that stops the keys going over as strings.
                sdk.nativeToScVal(
                    {
                        php_per_xlm_centavos: BigInt(opts.quote!.php_per_xlm_centavos),
                        php_per_usd_centavos: BigInt(opts.quote!.php_per_usd_centavos),
                        usd_per_xlm_e8: BigInt(opts.quote!.usd_per_xlm_e8),
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
            : contract.call(opts.action, loanKey)

        const built = new sdk.TransactionBuilder(account, {
            fee: sdk.BASE_FEE,
            networkPassphrase: NETWORK_PASSPHRASE,
        })
            .addOperation(operation)
            .setTimeout(180)
            .build()

        // Simulation attaches the footprint, the admin auth entry and the real
        // resource fee — and runs the contract's own checks, so an ordering or
        // price refusal usually surfaces here, before anyone signs anything.
        const prepared = await server.prepareTransaction(built)

        const signed = await signTransaction(prepared.toXDR(), {
            networkPassphrase: NETWORK_PASSPHRASE,
            address: signer,
        })
        if (signed.error || !signed.signedTxXdr) {
            return { error: signed.error?.message ?? 'Signing was cancelled' }
        }

        const sendResponse = await server.sendTransaction(
            sdk.TransactionBuilder.fromXDR(signed.signedTxXdr, NETWORK_PASSPHRASE),
        )
        if (sendResponse.status === 'ERROR') {
            return { error: refusalFrom(sendResponse.errorResult) ?? 'The network rejected the movement' }
        }

        for (let attempt = 0; attempt < 30; attempt++) {
            await sleep(2000)
            const result = await server.getTransaction(sendResponse.hash)
            if (result.status === 'SUCCESS') return { txHash: sendResponse.hash }
            if (result.status === 'FAILED') {
                return { error: refusalFrom(result.resultXdr) ?? 'The movement failed on-chain' }
            }
        }
        return { error: 'Still waiting for the network — if your wallet shows it went through, press Sign again to re-check' }
    } catch (err) {
        return {
            error: refusalFrom(err)
                ?? (err instanceof Error ? err.message : 'Unable to submit the movement'),
        }
    }
}
