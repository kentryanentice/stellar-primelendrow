import { useEffect, useMemo, useState } from 'react'
import useBorrow, { type GuarantorAsk } from './useBorrow'
import { parsePesoInput } from './money'
import type { PolicyParams, Product } from './types'

export type GuarantorRow = { username: string; pledge: string }

/**
 * The apply wizard's state and derived numbers: product -> amount/term ->
 * the ENGINE's quote (rate, cap, collateral requirement, and the borrower's
 * own cover floor — displayed verbatim, never computed here) ->
 * product-specific backing (a wallet for either coin leg, the borrower's own
 * deposit/XLM split and invitees for guarantor) -> recorded consent ->
 * submit.
 *
 * Lifted out of BorrowCard (rather than owned there) so the Borrow page's
 * sibling "Your eligibility" card can read the same live product/quote
 * without a second debounced quote request racing this one.
 * `use`-prefixed per this repo's React Compiler requirement.
 */
export default function useBorrowForm(params: PolicyParams, onChanged: () => void) {
    const borrow = useBorrow(onChanged)

    const [product, setProduct] = useState<Product>('deposit_backed')
    const [amountInput, setAmountInput] = useState('')
    const [term, setTerm] = useState(params.term_months.min)
    const [guarantorRows, setGuarantorRows] = useState<GuarantorRow[]>([{ username: '', pledge: '' }])
    const [consented, setConsented] = useState(false)
    /** Guarantor loans: how much of the borrower's own half each of their two
     *  legs carries. Peso text, parsed the same way the amount is. */
    const [depositCoverInput, setDepositCoverInput] = useState('')
    const [xlmCoverInput, setXlmCoverInput] = useState('')

    const amountCentavos = parsePesoInput(amountInput)

    useEffect(() => {
        borrow.requestQuote(product, amountCentavos, term)
        // requestQuote is stable; re-quote whenever the ask changes
    }, [borrow.requestQuote, product, amountCentavos, term]) // eslint-disable-line react-hooks/exhaustive-deps

    const productQuote = borrow.quote?.products.find(p => p.product === product) ?? null
    const overCap = amountCentavos !== null && productQuote !== null && amountCentavos > productQuote.max_amount

    const guarantorAsks: GuarantorAsk[] | null = useMemo(() => {
        if (product !== 'guarantor') return []
        const asks: GuarantorAsk[] = []
        for (const row of guarantorRows) {
            if (!row.username.trim()) continue
            const pledge = parsePesoInput(row.pledge)
            if (!pledge) return null // a named guarantor with no valid pledge
            asks.push({ username: row.username.trim(), pledge_amount: pledge })
        }
        return asks
    }, [product, guarantorRows])

    // ---- the borrower's own half (SOW §4.1, the 50% rule) ----
    //
    // Every rule below is re-derived by the engine at apply time; these are
    // the screen's copy, so the member is told what is wrong before they
    // submit rather than by a 422. `cover_required` itself is never computed
    // here — it is the engine's number, read off the quote.
    const depositCover = parsePesoInput(depositCoverInput) ?? 0
    const xlmCover = parsePesoInput(xlmCoverInput) ?? 0
    const coverTotal = depositCover + xlmCover
    const coverRequired = productQuote?.cover_required ?? null

    const coverShort =
        product === 'guarantor' && coverRequired !== null && coverTotal < coverRequired
    // Covering the whole loan is a deposit or XLM loan, not a guarantor one —
    // the engine refuses it, so the screen says so first.
    const coverLeavesNoGap =
        product === 'guarantor' && amountCentavos !== null && coverTotal >= amountCentavos
    /** What the guarantors are being asked for: the rest. */
    const guarantorGap =
        product === 'guarantor' && amountCentavos !== null
            ? Math.max(amountCentavos - coverTotal, 0)
            : null

    const pledgesTotal = (guarantorAsks ?? []).reduce((sum, g) => sum + g.pledge_amount, 0)
    const pledgesShort = product === 'guarantor' && guarantorGap !== null && pledgesTotal < guarantorGap

    /** A guarantor loan with a coin leg needs a wallet, exactly as an XLM loan
     *  does. BorrowCard owns the wallet list, so it gates on this. */
    const needsWallet = product === 'xlm_collateral' || (product === 'guarantor' && xlmCover > 0)

    const canSubmit =
        !borrow.applying
        && consented
        && amountCentavos !== null
        && amountCentavos >= params.min_loan
        && productQuote?.eligible === true
        && !overCap
        && (product !== 'guarantor' || (
            guarantorAsks !== null
            && guarantorAsks.length > 0
            && !pledgesShort
            && !coverShort
            && !coverLeavesNoGap
        ))

    /**
     * `walletId` is passed in rather than held here: the wallet list lives in
     * BorrowCard, because reading it drags in the wallet kit and that card is
     * lazy-loaded to keep the kit out of the initial bundle. Holding an id
     * here that only the card can resolve is what produced the bug this
     * replaced — the form stored '' while the card's <select> displayed the
     * one wallet a member had, so Apply stayed disabled until they changed
     * the dropdown, which with a single wallet they could never do.
     */
    const submit = async (walletId: string) => {
        if (!canSubmit || amountCentavos === null) return
        if (needsWallet && !walletId) return
        const applied = await borrow.apply({
            product,
            amount: amountCentavos,
            term_months: term,
            // A wallet goes with either coin leg — the whole of an XLM loan,
            // or the borrower's own XLM share of a guarantor loan.
            ...(needsWallet ? { wallet_id: walletId } : {}),
            ...(product === 'guarantor' ? {
                guarantors: guarantorAsks ?? [],
                deposit_cover: depositCover,
                xlm_cover: xlmCover,
            } : {}),
        })
        if (applied) {
            setAmountInput('')
            setConsented(false)
            setGuarantorRows([{ username: '', pledge: '' }])
            setDepositCoverInput('')
            setXlmCoverInput('')
        }
        return applied
    }

    return {
        ...borrow,
        product, setProduct,
        amountInput, setAmountInput,
        term, setTerm,
        guarantorRows, setGuarantorRows,
        consented, setConsented,
        depositCoverInput, setDepositCoverInput,
        xlmCoverInput, setXlmCoverInput,
        amountCentavos,
        productQuote, overCap,
        guarantorAsks, pledgesTotal, pledgesShort,
        depositCover, xlmCover, coverTotal, coverRequired,
        coverShort, coverLeavesNoGap, guarantorGap, needsWallet,
        canSubmit, submit,
    }
}

export type BorrowFormState = ReturnType<typeof useBorrowForm>
