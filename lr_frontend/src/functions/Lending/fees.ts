import type { RailFees } from './types'

/**
 * Payment-provider fee ESTIMATES for display, from the engine's own policy
 * (engine 043). These mirror `domain::receive_fee_estimate`, `gross_up` and
 * `payout_fee` exactly, so what the page shows before paying is what the
 * engine will charge — but the engine does the charging, and a deposit is
 * credited against the fee the provider actually reports, which can differ
 * by a few pesos. Nothing here is ever sent back to the engine.
 */

/** Banker's rounding, same as the engine's single rounding site. */
const roundHalfEven = (numer: number, denom: number) => {
    const quot = Math.floor(numer / denom)
    const rem = numer - quot * denom
    if (rem * 2 > denom) return quot + 1
    if (rem * 2 < denom) return quot
    return quot % 2 === 0 ? quot : quot + 1
}

/** What the provider is expected to keep when `charged` centavos are paid in. */
export const receiveFee = (charged: number, fees: RailFees) =>
    roundHalfEven(Math.max(0, charged) * fees.receive_bps, 10_000) + fees.receive_fixed

/** The checkout total that leaves exactly `applies` after the expected fee. */
export function grossUp(applies: number, fees: RailFees): { total: number; fee: number } {
    let total = Math.ceil(((applies + fees.receive_fixed) * 10_000) / (10_000 - fees.receive_bps))
    while (total > applies && total - 1 - receiveFee(total - 1, fees) >= applies) total -= 1
    while (total - receiveFee(total, fees) < applies) total += 1
    return { total, fee: total - applies }
}

/** The fee deducted from a payout of `amount`. */
export function payoutFee(amount: number, fees: RailFees) {
    let fee = roundHalfEven(Math.max(0, amount) * fees.payout_bps, 10_000)
    if (fees.payout_cap > 0) fee = Math.min(fee, fees.payout_cap)
    return Math.max(0, Math.min(fee, amount - 1))
}
