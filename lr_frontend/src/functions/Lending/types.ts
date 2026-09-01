/**
 * Wire types for the lending endpoints — a 1:1 replica of what lr_engine
 * serves (api/lending). The frontend never computes money from these; it
 * displays the engine's numbers verbatim (the UI is a window, never a
 * calculator). All peso amounts are whole centavos; all XLM amounts are
 * stroops; all rates are monthly basis points.
 */

export type Band = {
    min_score: number
    max_score: number
    cap: number
    secured_bps: number
    guarantor_bps: number
}

export type PolicyParams = {
    bands: Band[]
    deposit_ltv_pct: number
    xlm_min_collateral_pct: number
    xlm_liquidation_pct: number
    guarantor_cap_multiple: number
    guarantors_max: number
    term_months: { min: number; max: number }
    min_deposit: number
    min_loan: number
    interest_split: { savers: number; platform: number; reserve: number }
}

/** One public price feed's contribution to the agreed XLM/PHP rate. */
export type PriceSource = {
    name: string
    centavos_per_xlm: number
    /** 'XLM/PHP' when quoted directly, 'XLM/USD x USD/PHP' when derived. */
    leg: string
    deviation_bps: number
    /** false = further than the engine's band from the median, so excluded. */
    used: boolean
}

/**
 * The XLM->PHP conversion behind every collateral number, with the evidence
 * for it. The engine agrees this from several independent public feeds and
 * refuses to issue a collateral loan when fewer than two of them agree —
 * `live: false` means no agreement was reached and this is the last rate on
 * record, safe to display and never enough to borrow against.
 */
export type FxQuote = {
    centavos_per_xlm: number
    /** Unix seconds the feeds were read. */
    as_of: number
    method: string
    usd_php_centavos: number | null
    sources: PriceSource[]
    failures: { name: string; reason: string }[]
    live: boolean
}

export type LotBadge = 'available' | 'lent' | 'collateral' | 'pledged'

export type Lot = {
    id: string
    amount: number
    badge: LotBadge
    backing_loan: string | null
    created_at: number
}

/** POST /pool/deposits — the caller's own lots, paginated (fixed page size). */
export type LotsPage = {
    items: Lot[]
    total: number
    page: number
    page_size: number
    total_pages: number
}

export type PoolResponse = {
    pool: {
        total_deposits: number
        cash_available: number
        out_on_loans: number
        active_loans: number
        utilization_pct: number
    }
    me: {
        available: number
        lent: number
        collateral: number
        pledged: number
        score: number
    }
    params: {
        policy: PolicyParams
        /** The agreed rate alone; `fx` is the same number with its provenance. */
        fx_centavos_per_xlm: number
        fx: FxQuote
        collateral_contract: string | null
        paypal_ready: boolean
    }
}

export type Product = 'deposit_backed' | 'xlm_collateral' | 'guarantor'

export type ProductQuote = {
    product: Product
    eligible: boolean
    reason: string | null
    rate_bps: number
    max_amount: number
    required_deposit: number | null
    required_stroops: number | null
    required_pledges: number | null
}

export type QuoteResponse = {
    score: number
    band_cap: number | null
    eligible: boolean
    products: ProductQuote[]
    schedule_preview: { installment: number; principal_due: number; interest_due: number }[] | null
    total_interest: number | null
    /** The conversion `required_stroops` was computed at. */
    fx: FxQuote
}

/**
 * The pinned rate in the shape the vault contract takes it. The wallet
 * submits these three numbers with the lock and the contract measures the
 * dollar leg against a public SEP-40 feed (Reflector), refusing a stale feed,
 * an out-of-band quote, or a peso rate the two legs don't support — so
 * nothing here is worth a client tampering with.
 */
export type PinnedQuote = {
    /** Whole centavos one XLM is worth: the rate the collateral is valued at. */
    priced_centavos_per_xlm: number | null
    /** USD per XLM scaled 1e8 — the leg the on-chain feed checks. */
    priced_usd_per_xlm_e8: number | null
    /** Whole centavos one USD is worth — the leg it was crossed through. */
    priced_usd_php_centavos: number | null
}

export type ApplyResponse = PinnedQuote & {
    loan_id: string
    status: 'active' | 'pending'
    rate_bps: number
    /** Whole centavos, as recorded by the engine — what the lock covers. */
    principal: number
    required_stroops: number | null
    collateral_contract: string | null
    /** xlm_collateral: when the rate was read, and how it was reconciled. */
    priced_at: number | null
    price_method: string | null
    /** The collateral ratio the contract enforces, in basis points. */
    collateral_ratio_bps: number | null
    message: string
}

/**
 * One movement of collateral in or out of the vault. `queued` means the
 * engine has recorded the intent but the admin key hasn't executed it yet —
 * a claim about us, not about the chain, and shown as such.
 */
export type CollateralMovement = {
    kind: 'lock' | 'mark_repaid' | 'release' | 'mark_defaulted' | 'seize'
    status: 'confirmed' | 'queued'
    tx_hash: string | null
    at: number | null
    /** A seizure is priced again at the day's checked quote; a lock uses the pinned one. */
    quote_php_per_xlm_centavos: number | null
    quote_usd_per_xlm_e8: number | null
    quote_php_per_usd_centavos: number | null
}

/** One public feed's contribution to the rate a position was struck at. */
export type CollateralPriceSource = {
    name: string
    centavos_per_xlm: number
    /** 'XLM/PHP' when quoted directly, 'XLM/USD x USD/PHP' when derived. */
    leg: string
    deviation_bps: number
    /** false = outside the engine's 5% band, so excluded from the median. */
    used: boolean
}

/**
 * GET /loans/{id}/collateral — the custody record: the position, the price it
 * was struck at with the feeds behind it, and every on-chain movement with
 * its transaction hash. Everything here is checkable against the ledger.
 */
export type CollateralRecord = {
    loan_id: string
    principal: number
    loan_status: Loan['status']
    /** The vault contract the record should be checked against. */
    contract_id: string | null
    wallet_address: string
    required_stroops: number
    locked_stroops: number
    status: 'pending' | 'locked' | 'released' | 'seized'
    collateral_ratio_bps: number
    created_at: number
    /** When the lock was verified on-chain; null while still pending. */
    locked_at: number | null
    /** Worth of the locked coins at the LIVE rate — display only. */
    value_centavos: number | null
    health_pct: number | null
    liquidatable: boolean
    price: {
        centavos_per_xlm: number | null
        usd_per_xlm_e8: number | null
        usd_php_centavos: number | null
        priced_at: number | null
        sources_used: number
        sources: CollateralPriceSource[]
    }
    movements: CollateralMovement[]
}

export type ScheduleRow = {
    installment: number
    due_at: number
    principal_due: number
    interest_due: number
    principal_paid: number
    interest_paid: number
    status: 'scheduled' | 'paid' | 'late' | 'defaulted'
}

export type Loan = {
    id: string
    product: Product
    principal: number
    rate_bps: number
    term_months: number
    status: 'pending' | 'active' | 'closed' | 'defaulted' | 'declined' | 'cancelled'
    principal_outstanding: number
    disbursed_at: number | null
    closed_at: number | null
    created_at: number
    schedule: ScheduleRow[]
    collateral: (PinnedQuote & {
        wallet_address: string
        required_stroops: number
        locked_stroops: number
        status: 'pending' | 'locked' | 'released' | 'seized'
        health_pct: number | null
        liquidatable: boolean
        /** Pinned at issuance; null on positions created before the oracle. */
        priced_at: number | null
        collateral_ratio_bps: number | null
    }) | null
    guarantors: { username: string; pledge_amount: number; status: string }[]
}

export type Payment = {
    id: number
    loan_id: string
    product: Product
    amount_received: number
    interest_paid: number
    principal_paid: number
    excess: number
    paid_at: number
}

export type Invite = {
    id: string
    loan_id: string
    borrower: string
    product: Product
    amount: number
    rate_bps: number
    term_months: number
    pledge_amount: number
    status: 'invited' | 'accepted' | 'declined' | 'released' | 'seized'
    created_at: number
}

export const PRODUCT_LABEL: Record<Product, string> = {
    deposit_backed: 'Deposit-backed loan',
    xlm_collateral: 'XLM-collateral loan',
    guarantor: 'Guarantor loan',
}
