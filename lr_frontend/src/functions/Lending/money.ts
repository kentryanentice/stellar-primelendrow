/**
 * Display-only money formatting. Division here is fine — nothing formatted
 * by this file is ever submitted back to the engine; every mutation posts
 * intent (an order id, whole centavos typed by the user) and the engine does
 * the arithmetic that counts.
 */

export const pesos = (centavos: number) =>
    `₱${(centavos / 100).toLocaleString('en-PH', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`

/** Compact form for stat tiles: no decimals when they'd be .00 noise. */
export const pesosCompact = (centavos: number) =>
    `₱${(centavos / 100).toLocaleString('en-PH', { maximumFractionDigits: centavos % 100 === 0 ? 0 : 2 })}`

export const xlm = (stroops: number) =>
    `${(stroops / 10_000_000).toLocaleString(undefined, { maximumFractionDigits: 7 })} XLM`

/** The XLM/PHP conversion, as a rate: "₱11.00 / XLM". */
export const xlmRate = (centavosPerXlm: number) => `${pesos(centavosPerXlm)} / XLM`

/** "12s ago" / "4m ago" — how old a price read is, in words. */
export function since(secs: number) {
    const age = Math.max(0, Math.floor(Date.now() / 1000) - secs)
    if (age < 60) return `${age}s ago`
    if (age < 3600) return `${Math.floor(age / 60)}m ago`
    if (age < 86400) return `${Math.floor(age / 3600)}h ago`
    return `${Math.floor(age / 86400)}d ago`
}

/** Monthly basis points -> "1.75%/mo". */
export const rate = (bps: number) => `${(bps / 100).toFixed(2).replace(/\.?0+$/, '')}%/mo`

/** "1,500.00" or "1500" typed by a user -> whole centavos, or null if not a
 *  clean peso amount. String math only — same rule as the backend parser. */
export function parsePesoInput(input: string): number | null {
    const cleaned = input.replace(/[,\s₱]/g, '')
    if (!cleaned) return null
    const match = /^(\d{1,10})(?:\.(\d{1,2}))?$/.exec(cleaned)
    if (!match) return null
    const whole = parseInt(match[1], 10)
    const frac = match[2] ? parseInt(match[2].padEnd(2, '0'), 10) : 0
    return whole * 100 + frac
}

export const formatDate = (secs: number) =>
    new Date(secs * 1000).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' })
