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

/** What the provider charges on top for a payout that sends `sent`. */
const payoutCharge = (sent: number, fees: RailFees) => {
    const fee = roundHalfEven(Math.max(0, sent) * fees.payout_bps, 10_000)
    return fees.payout_cap > 0 ? Math.min(fee, fees.payout_cap) : fee
}

/** The fee deducted from a payout claim of `amount`: what's sent plus the
 *  provider's charge on it fits exactly inside the claim (engine 044). */
export function payoutFee(amount: number, fees: RailFees) {
    if (amount <= 1) return 0
    let sent = Math.floor((amount * 10_000) / (10_000 + fees.payout_bps))
    if (fees.payout_cap > 0) sent = Math.max(sent, amount - fees.payout_cap)
    sent = Math.min(Math.max(sent, 1), amount)
    while (sent > 1 && sent + payoutCharge(sent, fees) > amount) sent -= 1
    while (sent < amount && sent + 1 + payoutCharge(sent + 1, fees) <= amount) sent += 1
    return amount - sent
}
