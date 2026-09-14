import type { InterestParts } from './types'

export type Recipient = keyof InterestParts

/**
 * Who receives interest, in the fixed order every split visual draws them:
 * the three fixed shares first, then the risk band's two halves side by side.
 * The order is also the color order (`.is-<key>` in the lending stylesheet),
 * so a recipient keeps its color on every chart and never shifts with rank.
 * Labels only. Percentages are read off the policy and every amount comes
 * from the engine.
 */
export const RECIPIENTS: { key: Recipient; label: string }[] = [
    { key: 'depositors', label: 'Depositors' },
    { key: 'reserve', label: 'Reserve' },
    { key: 'platform', label: 'Platform' },
    { key: 'guarantor', label: 'Guarantor' },
    { key: 'recovery_fund', label: 'Recovery fund' },
]

/** A part's width in a 100% bar. Drawing geometry only, never a money figure. */
export const widthPct = (part: number, interest: number) => (interest > 0 ? (part / interest) * 100 : 0)
