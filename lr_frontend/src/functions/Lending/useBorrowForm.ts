import { useEffect, useMemo, useState } from 'react'
import useBorrow, { type GuarantorAsk } from './useBorrow'
import { parsePesoInput } from './money'
import type { PolicyParams, Product } from './types'

export type GuarantorRow = { username: string; pledge: string }

/**
 * The apply wizard's state and derived numbers: product -> amount/term ->
 * the ENGINE's quote (rate, cap, collateral requirement — displayed
 * verbatim, never computed here) -> product-specific backing (wallet for
 * XLM, invitees for guarantor) -> recorded consent -> submit.
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
    const [walletId, setWalletId] = useState('')
    const [consented, setConsented] = useState(false)

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

    const pledgesTotal = (guarantorAsks ?? []).reduce((sum, g) => sum + g.pledge_amount, 0)
    const pledgesShort = product === 'guarantor' && amountCentavos !== null && pledgesTotal < amountCentavos

    const canSubmit =
        !borrow.applying
        && consented
        && amountCentavos !== null
        && amountCentavos >= params.min_loan
        && productQuote?.eligible === true
        && !overCap
        && (product !== 'xlm_collateral' || walletId !== '')
        && (product !== 'guarantor' || (guarantorAsks !== null && guarantorAsks.length > 0 && !pledgesShort))

    const submit = async () => {
        if (!canSubmit || amountCentavos === null) return
        const applied = await borrow.apply({
            product,
            amount: amountCentavos,
            term_months: term,
            ...(product === 'xlm_collateral' ? { wallet_id: walletId } : {}),
            ...(product === 'guarantor' ? { guarantors: guarantorAsks ?? [] } : {}),
        })
        if (applied) {
            setAmountInput('')
            setConsented(false)
            setGuarantorRows([{ username: '', pledge: '' }])
        }
        return applied
    }

    return {
        ...borrow,
        product, setProduct,
        amountInput, setAmountInput,
        term, setTerm,
        guarantorRows, setGuarantorRows,
        walletId, setWalletId,
        consented, setConsented,
        amountCentavos,
        productQuote, overCap,
        guarantorAsks, pledgesTotal, pledgesShort,
        canSubmit, submit,
    }
}

export type BorrowFormState = ReturnType<typeof useBorrowForm>
