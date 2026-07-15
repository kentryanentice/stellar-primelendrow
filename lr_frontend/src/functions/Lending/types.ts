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
        fx_centavos_per_xlm: number
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
}

export type ApplyResponse = {
    loan_id: string
    status: 'active' | 'pending'
    rate_bps: number
    required_stroops: number | null
    collateral_contract: string | null
    message: string
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
    collateral: {
        wallet_address: string
        required_stroops: number
        locked_stroops: number
        status: 'pending' | 'locked' | 'released' | 'seized'
        health_pct: number | null
        liquidatable: boolean
    } | null
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
