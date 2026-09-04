/**
 * Links out to Stellar Expert, and the shortening that makes a 64-character
 * hash fit in a row.
 *
 * Shared rather than redefined per card: three places now render an on-chain
 * reference — the borrower's custody record, their transaction record, and the
 * operator's loan book — and a link that pointed at the wrong network on one
 * of them would be worse than no link at all.
 */

const NETWORK = import.meta.env.VITE_STELLAR_NETWORK === 'public' ? 'public' : 'testnet'

export const txLink = (hash: string) => `https://stellar.expert/explorer/${NETWORK}/tx/${hash}`
export const contractLink = (id: string) => `https://stellar.expert/explorer/${NETWORK}/contract/${id}`
export const accountLink = (address: string) => `https://stellar.expert/explorer/${NETWORK}/account/${address}`

/** "GABC…WXYZ" — enough to recognise an id, short enough to sit in a row. */
export const shortId = (value: string) =>
    value.length > 16 ? `${value.slice(0, 5)}…${value.slice(-4)}` : value
