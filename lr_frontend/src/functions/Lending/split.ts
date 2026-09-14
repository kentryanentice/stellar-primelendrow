import type { InterestParts, InterestSplit } from './types'

export type Recipient = keyof InterestParts

/**
 * Who receives interest, in the fixed order every split visual draws them:
 * the three fixed shares first, then the risk band's two halves side by side.
 * The order is also the color order (`.is-<key>` in the lending stylesheet),
 * so a recipient keeps its color on every chart and never shifts with rank.
 * Labels and wording only. Percentages are read off the policy and every
 * amount comes from the engine.
 */
export const RECIPIENTS: { key: Recipient; label: string; blurb: (s: InterestSplit) => string }[] = [
    { key: 'depositors', label: 'Depositors', blurb: s => `Fixed ${s.depositors}%, shared by everyone whose deposit funds the loan, in proportion to their deposit.` },
    { key: 'reserve', label: 'Lending reserve', blurb: s => `Fixed ${s.reserve}%, held back to keep the pool lending.` },
    { key: 'platform', label: 'Platform fee', blurb: s => `Fixed ${s.platform}%, for running PrimeLendRow.` },
    { key: 'guarantor', label: 'Guarantor', blurb: s => `Paid from the ${s.risk_band}% risk band by score tier, capped at ${s.guarantor_cap}%.` },
    { key: 'recovery_fund', label: 'Recovery fund', blurb: () => 'The rest of the risk band. It absorbs defaults.' },
]

/** A part's width in a 100% bar. Drawing geometry only, never a money figure. */
export const widthPct = (part: number, interest: number) => (interest > 0 ? (part / interest) * 100 : 0)
