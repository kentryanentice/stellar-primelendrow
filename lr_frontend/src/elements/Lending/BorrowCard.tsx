import { useMemo } from 'react'
import { HandCoins, Plus, Trash2, ShieldCheck } from 'lucide-react'
import useWallets from '../../functions/Wallet/useWallets'
import { truncateAddress } from '../../functions/Wallet/wallet'
import { pesos, rate, xlm } from '../../functions/Lending/money'
import { PRODUCT_LABEL, type Loan, type PoolResponse, type Product } from '../../functions/Lending/types'
import type { BorrowFormState } from '../../functions/Lending/useBorrowForm'

/**
 * The apply wizard's form: product -> amount/term -> the ENGINE's quote
 * (rate, cap, collateral requirement — displayed verbatim, never computed
 * here) -> product-specific backing (wallet for XLM, invitees for
 * guarantor) -> recorded consent -> submit. A tampered screen can mislead
 * its own user, but the engine re-derives every number at apply time
 * regardless. Purely a rendering of `form` (useBorrowForm, owned by the
 * Borrow page) — the sibling "Your eligibility" card reads the same state.
 */
function BorrowCard({ data, form, openLoan }: { data: PoolResponse; form: BorrowFormState; openLoan: Loan | null }) {
    const { params } = data
    const { wallets } = useWallets()
    const {
        product, setProduct,
        amountInput, setAmountInput,
        term, setTerm,
        guarantorRows, setGuarantorRows,
        walletId, setWalletId,
        consented, setConsented,
        amountCentavos,
        productQuote, overCap,
        pledgesTotal, pledgesShort,
        canSubmit, submit,
        quote, quoting, applying,
        pendingLock, setPendingLock, lockAndConfirm, locking,
    } = form

    const activeWallets = useMemo(() => wallets.filter(w => w.status === 'active'), [wallets])
    const lockWallet = activeWallets.find(w => w.id === walletId) ?? activeWallets[0]

    // A fresh XLM application: the wizard's last step is the on-chain lock.
    if (pendingLock) {
        return (
            <section className='lending-card lending-card-borrow'>
                <div className='lending-card-head'>
                    <span className='lending-card-icon is-accent'><ShieldCheck /></span>
                    <h2>Lock your collateral</h2>
                </div>
                <p className='lending-muted'>
                    Your loan is approved pending collateral. Lock{' '}
                    <b>{xlm(pendingLock.required_stroops ?? 0)}</b> from your wallet into the vault contract —
                    the engine verifies the transaction on the network, then disburses. Only the platform can
                    release or seize the vault; your coins come back automatically when the loan is repaid.
                </p>
                <button
                    type='button'
                    className='lending-btn-primary'
                    disabled={locking || !lockWallet}
                    onClick={() => lockWallet && lockAndConfirm(lockWallet.address)}
                >
                    {locking ? 'Locking on-chain…' : 'Lock with Freighter'}
                </button>
                <button
                    type='button'
                    className='lending-btn'
                    disabled={locking}
                    onClick={() => setPendingLock(null)}
                >
                    Later — it stays pending in “Your loans”
                </button>
            </section>
        )
    }

    // The engine's own reason ("You already have an open loan — repay it
    // first") is generic — when the blocker IS the open loan we already have
    // on hand (principal_outstanding), name the actual amount instead. Falls
    // back to the engine's string for any other ineligibility reason.
    const blocked = quote && !quote.eligible
    const blockedReason = blocked
        ? (openLoan ? `Open loan active — submission unlocks once ${pesos(openLoan.principal_outstanding)} is repaid.` : productQuote?.reason ?? 'You already have an open loan — repay it first.')
        : null

    return (
        <section className='lending-card lending-card-borrow'>
            <div className='lending-card-head'>
                <span className='lending-card-icon is-accent'><HandCoins /></span>
                <h2>Apply for your next loan</h2>
            </div>
            <p className='lending-muted'>Set it up now — choose how you’ll back it and see your rate before you commit.</p>

            {/* Surfaced as soon as the (auto-fired) quote comes back, before the
                user's typed anything — same block the engine gives per-product
                in the quote box below, just shown up front instead of buried
                there once they've already started filling the form in. */}
            {blockedReason && (
                <p className='lending-field-error lending-borrow-blocked'>
                    <span className='lending-locked-badge'>Locked</span> {blockedReason}
                </p>
            )}

            <div className='lending-product-tiles' role='radiogroup' aria-label='Loan product'>
                {(Object.keys(PRODUCT_LABEL) as Product[]).map(key => {
                    const preview = quote?.products.find(p => p.product === key)
                    return (
                        <button
                            key={key}
                            type='button'
                            role='radio'
                            aria-checked={product === key}
                            className={`lending-product-tile${product === key ? ' is-selected' : ''}`}
                            onClick={() => setProduct(key)}
                        >
                            <span className='lending-product-tile-label'>{PRODUCT_LABEL[key]}</span>
                            <span className='lending-product-tile-rate'>{preview ? rate(preview.rate_bps) : '—'}</span>
                            <span className='lending-muted'>{preview ? `Up to ${pesos(preview.max_amount)}` : 'Enter an amount to preview'}</span>
                        </button>
                    )
                })}
            </div>

            <p className='lending-muted'>
                {product === 'deposit_backed' && `Borrow against your own deposit — up to ${params.policy.deposit_ltv_pct}% of what's withdrawable, at the secured rate.`}
                {product === 'xlm_collateral' && `Lock XLM worth at least ${params.policy.xlm_min_collateral_pct}% of the loan in the vault contract. Falling under ${params.policy.xlm_liquidation_pct}% risks liquidation.`}
                {product === 'guarantor' && `No deposit or collateral needed — up to ${params.policy.guarantors_max} verified members pledge their deposits for you, and your cap doubles.`}
            </p>

            <div className='lending-borrow-split'>
                <div className='lending-borrow-fields'>
                    <div className='lending-borrow-form'>
                        <div className='lending-field'>
                            <label className='lending-label' htmlFor='lending-borrow-amount'>Loan amount</label>
                            <input
                                id='lending-borrow-amount'
                                className='lending-input'
                                inputMode='decimal'
                                placeholder={`Min ${pesos(params.policy.min_loan)}`}
                                value={amountInput}
                                onChange={e => setAmountInput(e.target.value)}
                            />
                        </div>
                        <div className='lending-field'>
                            <label className='lending-label' htmlFor='lending-borrow-term'>Term</label>
                            <select
                                id='lending-borrow-term'
                                className='lending-input'
                                value={term}
                                onChange={e => setTerm(Number(e.target.value))}
                            >
                                {Array.from(
                                    { length: params.policy.term_months.max - params.policy.term_months.min + 1 },
                                    (_, i) => params.policy.term_months.min + i,
                                ).map(months => (
                                    <option key={months} value={months}>{months} months</option>
                                ))}
                            </select>
                        </div>
                    </div>

                    {/* ---- product-specific backing ---- */}
                    {product === 'xlm_collateral' && (
                        <div className='lending-field'>
                            <label className='lending-label' htmlFor='lending-borrow-wallet'>Wallet that will lock the XLM</label>
                            {activeWallets.length === 0 ? (
                                <p className='lending-muted'>Connect a wallet in Settings first.</p>
                            ) : (
                                <select
                                    id='lending-borrow-wallet'
                                    className='lending-input'
                                    value={walletId || activeWallets[0].id}
                                    onChange={e => setWalletId(e.target.value)}
                                >
                                    {activeWallets.map(w => (
                                        <option key={w.id} value={w.id}>
                                            {w.label ? `${w.label} — ` : ''}{truncateAddress(w.address)}
                                        </option>
                                    ))}
                                </select>
                            )}
                        </div>
                    )}

                    {product === 'guarantor' && (
                        <div className='lending-guarantor-asks'>
                            <p className='lending-label'>Guarantors (up to {params.policy.guarantors_max})</p>
                            {guarantorRows.map((row, i) => (
                                <div key={i} className='lending-guarantor-row'>
                                    <input
                                        className='lending-input'
                                        placeholder='Member username'
                                        value={row.username}
                                        onChange={e => setGuarantorRows(rows => rows.map((r, j) => j === i ? { ...r, username: e.target.value } : r))}
                                    />
                                    <input
                                        className='lending-input'
                                        inputMode='decimal'
                                        placeholder='Pledge ₱'
                                        value={row.pledge}
                                        onChange={e => setGuarantorRows(rows => rows.map((r, j) => j === i ? { ...r, pledge: e.target.value } : r))}
                                    />
                                    {guarantorRows.length > 1 && (
                                        <button
                                            type='button'
                                            className='lending-icon-btn'
                                            aria-label='Remove guarantor'
                                            onClick={() => setGuarantorRows(rows => rows.filter((_, j) => j !== i))}
                                        >
                                            <Trash2 />
                                        </button>
                                    )}
                                </div>
                            ))}
                            {guarantorRows.length < params.policy.guarantors_max && (
                                <button
                                    type='button'
                                    className='lending-btn lending-btn-add'
                                    onClick={() => setGuarantorRows(rows => [...rows, { username: '', pledge: '' }])}
                                >
                                    <Plus /> Add guarantor
                                </button>
                            )}
                            {pledgesShort && (
                                <p className='lending-field-error'>
                                    Pledges add up to {pesos(pledgesTotal)} — they must cover the full {amountCentavos !== null ? pesos(amountCentavos) : 'amount'}.
                                </p>
                            )}
                        </div>
                    )}

                    {/* ---- consent (D1): recorded server-side with the application ---- */}
                    <label className='lending-consent'>
                        <input
                            type='checkbox'
                            checked={consented}
                            onChange={e => setConsented(e.target.checked)}
                        />
                        <span>
                            {product === 'deposit_backed' && 'I authorize PrimeLendRow to hold the loan amount from my deposit as security until repayment.'}
                            {product === 'xlm_collateral' && 'I agree to lock my XLM in the vault contract and accept liquidation if coverage falls below the threshold.'}
                            {product === 'guarantor' && 'I confirm my guarantors have agreed to pledge their deposits toward this loan.'}
                        </span>
                    </label>

                    <button type='button' className='lending-btn-primary' disabled={!canSubmit} onClick={submit}>
                        {applying ? 'Submitting…' : 'Apply for this loan'}
                    </button>
                </div>

                <div className='lending-borrow-quotepanel'>
                    <span className='lending-stat-label'>Your quote</span>
                    {productQuote ? (
                        <div className='lending-quote-rows'>
                            {quote?.schedule_preview?.[0] && (
                                <div className='lending-quote-row'>
                                    <span>Monthly payment</span>
                                    <b>{pesos(quote.schedule_preview[0].principal_due + quote.schedule_preview[0].interest_due)}</b>
                                </div>
                            )}
                            {quote?.total_interest != null && (
                                <div className='lending-quote-row'>
                                    <span>Total interest over {term} months</span>
                                    <b>{pesos(quote.total_interest)}</b>
                                </div>
                            )}
                            <div className='lending-quote-row'>
                                <span>Rate</span>
                                <b className='is-good'>{rate(productQuote.rate_bps)}</b>
                            </div>
                            {product === 'deposit_backed' && amountCentavos !== null && productQuote.required_deposit !== null && (
                                <div className='lending-quote-row'>
                                    <span>Deposit that locks as collateral</span>
                                    <b>{pesos(productQuote.required_deposit)}</b>
                                </div>
                            )}
                            {product === 'xlm_collateral' && amountCentavos !== null && productQuote.required_stroops !== null && (
                                <div className='lending-quote-row'>
                                    <span>XLM to lock ({params.policy.xlm_min_collateral_pct}%)</span>
                                    <b>{xlm(productQuote.required_stroops)}</b>
                                </div>
                            )}
                            {product === 'guarantor' && productQuote.required_pledges !== null && (
                                <div className='lending-quote-row'>
                                    <span>Pledges needed</span>
                                    <b>{pesos(productQuote.required_pledges)}</b>
                                </div>
                            )}
                        </div>
                    ) : (
                        <p className='lending-muted'>Enter an amount to see your quote.</p>
                    )}
                    {overCap && <p className='lending-field-error'>That's over your cap for this product.</p>}
                    {quoting && <p className='lending-muted'>Updating quote…</p>}
                </div>
            </div>
        </section>
    )
}

export default BorrowCard
