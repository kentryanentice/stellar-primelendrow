import { useMemo, useState } from 'react'
import { HandCoins, Plus, Trash2, ShieldCheck } from 'lucide-react'
import useWallets from '../../functions/Wallet/useWallets'
import { truncateAddress } from '../../functions/Wallet/wallet'
import { pesos, rate, since, xlm, xlmRate } from '../../functions/Lending/money'
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
        consented, setConsented,
        depositCoverInput, setDepositCoverInput,
        xlmCoverInput, setXlmCoverInput,
        amountCentavos,
        productQuote, overCap,
        pledgesTotal, pledgesShort,
        coverTotal, coverRequired, coverShort, coverLeavesNoGap,
        guarantorGap, needsWallet,
        canSubmit, submit,
        quote, quoting, applying,
        pendingLock, setPendingLock, lockAndConfirm, locking,
    } = form

    const activeWallets = useMemo(() => wallets.filter(w => w.status === 'active'), [wallets])

    // Only what the member has actively picked. The effective choice below
    // falls back to the first wallet — which is what the <select> shows — so
    // a member with exactly one wallet doesn't have to change a dropdown that
    // has nothing to change to before Apply will enable.
    const [chosenWallet, setChosenWallet] = useState('')
    const walletId = chosenWallet || activeWallets[0]?.id || ''
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
                    the engine verifies the transaction on the network
                    {/* A guarantor loan is waiting on two legs, not one: say
                        so rather than promising a disbursement this step
                        alone will not trigger. */}
                    {pendingLock.guarantor_gap !== null
                        ? ', and disburses once your guarantors have accepted their share too'
                        : ', then disburses'}. Only the platform can release or seize the vault; your coins come
                    back automatically when the loan is repaid.
                </p>
                {pendingLock.guarantor_gap !== null && pendingLock.cover_xlm !== null && (
                    <p className='lending-muted'>
                        These coins carry <b>{pesos(pendingLock.cover_xlm)}</b> of your own share, locked at{' '}
                        {params.policy.xlm_min_collateral_pct}% — your guarantors cover the remaining{' '}
                        <b>{pesos(pendingLock.guarantor_gap)}</b>.
                    </p>
                )}
                {/* The rate this requirement was struck at is pinned to the
                    loan, so it is worth naming here: a later price move will
                    not change the amount being asked for. */}
                {pendingLock.priced_centavos_per_xlm !== null && (
                    <p className='lending-muted'>
                        Priced at <b>{xlmRate(pendingLock.priced_centavos_per_xlm)}</b>
                        {pendingLock.price_method ? ` — ${pendingLock.price_method}` : ''}
                        {pendingLock.priced_at !== null ? `, read ${since(pendingLock.priced_at)}` : ''}. This
                        rate is locked to your loan and won't be recalculated. The vault checks it against a
                        public price feed before it accepts your coins, and refuses the lock if the feed has
                        gone quiet or the market has moved too far from it.
                    </p>
                )}
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
                {product === 'guarantor' && `You cover at least ${params.policy.borrower_cover_min_pct}% yourself — from your deposit, your XLM, or both — and up to ${params.policy.guarantors_max} verified members pledge for the rest. Your cap doubles.`}
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

                    {/* ---- product-specific backing ----
                        The wallet select follows the coin leg, not the
                        product: a guarantor loan needs one too once the
                        borrower puts part of their own half in XLM. */}
                    {needsWallet && (
                        <div className='lending-field'>
                            <label className='lending-label' htmlFor='lending-borrow-wallet'>Wallet that will lock the XLM</label>
                            {activeWallets.length === 0 ? (
                                <p className='lending-muted'>Connect a wallet in Settings first.</p>
                            ) : (
                                <select
                                    id='lending-borrow-wallet'
                                    className='lending-input'
                                    value={walletId}
                                    onChange={e => setChosenWallet(e.target.value)}
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
                            {/* ---- the borrower's own half, before anyone is
                                asked to vouch. The floor is the engine's
                                number; the two legs are the member's choice
                                of how to carry it. ---- */}
                            <p className='lending-label'>
                                Your own share
                                {coverRequired !== null && <> — at least <b>{pesos(coverRequired)}</b></>}
                            </p>
                            <p className='lending-muted'>
                                You cover {params.policy.borrower_cover_min_pct}% of this loan yourself before your
                                guarantors are asked for anything, and you're always the first to be charged if it
                                defaults. Split it however you like between the two.
                            </p>
                            <div className='lending-borrow-form'>
                                <div className='lending-field'>
                                    <label className='lending-label' htmlFor='lending-cover-deposit'>From your deposit</label>
                                    <input
                                        id='lending-cover-deposit'
                                        className='lending-input'
                                        inputMode='decimal'
                                        placeholder='₱0'
                                        value={depositCoverInput}
                                        onChange={e => setDepositCoverInput(e.target.value)}
                                    />
                                    {productQuote?.cover_max_from_deposit != null && (
                                        <small className='lending-muted'>
                                            {productQuote.cover_max_from_deposit > 0
                                                ? `Up to ${pesos(productQuote.cover_max_from_deposit)} withdrawable`
                                                : 'No withdrawable deposit — cover it in XLM instead'}
                                        </small>
                                    )}
                                </div>
                                <div className='lending-field'>
                                    <label className='lending-label' htmlFor='lending-cover-xlm'>From your XLM</label>
                                    <input
                                        id='lending-cover-xlm'
                                        className='lending-input'
                                        inputMode='decimal'
                                        placeholder='₱0'
                                        value={xlmCoverInput}
                                        onChange={e => setXlmCoverInput(e.target.value)}
                                    />
                                    {/* The exact stroops depend on the split,
                                        and the engine prices them at apply —
                                        so this names the ratio and the
                                        all-XLM endpoint rather than inventing
                                        a figure for a partial leg. */}
                                    <small className='lending-muted'>
                                        Locked at {params.policy.xlm_min_collateral_pct}% of whatever you put here
                                        {productQuote?.cover_stroops_if_all_xlm != null && coverRequired !== null
                                            ? ` — ${xlm(productQuote.cover_stroops_if_all_xlm)} to carry all ${pesos(coverRequired)}`
                                            : ''}.
                                    </small>
                                </div>
                            </div>
                            {coverShort && coverRequired !== null && (
                                <p className='lending-field-error'>
                                    Your share adds up to {pesos(coverTotal)} — it must be at least {pesos(coverRequired)}.
                                </p>
                            )}
                            {coverLeavesNoGap && (
                                <p className='lending-field-error'>
                                    You're covering the whole loan yourself — apply for a deposit-backed or XLM loan
                                    instead, and skip the guarantors.
                                </p>
                            )}

                            <p className='lending-label'>
                                Guarantors (up to {params.policy.guarantors_max})
                                {guarantorGap !== null && !coverLeavesNoGap && <> — for the remaining <b>{pesos(guarantorGap)}</b></>}
                            </p>
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
                            {pledgesShort && !coverLeavesNoGap && (
                                <p className='lending-field-error'>
                                    Pledges add up to {pesos(pledgesTotal)} — they must cover the {guarantorGap !== null ? pesos(guarantorGap) : 'amount'} you aren't covering yourself.
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
                            {product === 'guarantor' && 'I authorize PrimeLendRow to hold my own share as security, and confirm my guarantors have agreed to pledge their deposits for the rest. My own share is charged first if this loan defaults.'}
                        </span>
                    </label>

                    {/* The wallet gate lives here, not in the form: this is
                        the only place that knows whether the member has one. */}
                    <button
                        type='button'
                        className='lending-btn-primary'
                        disabled={!canSubmit || (needsWallet && !walletId)}
                        onClick={() => void submit(walletId)}
                    >
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
                            {/* The conversion behind that number, not just its
                                result: the engine agrees the rate across
                                independent public feeds, so the borrower can
                                see which price they're being asked to lock at
                                and how fresh it is. */}
                            {needsWallet && quote?.fx && (
                                <div className='lending-quote-row lending-quote-fx'>
                                    <span>
                                        Rate used
                                        <small className='lending-muted'>
                                            {quote.fx.live
                                                ? ` ${quote.fx.method}, ${since(quote.fx.as_of)}`
                                                : ' last recorded rate — no live feed agreed'}
                                        </small>
                                    </span>
                                    <b className={quote.fx.live ? undefined : 'is-warn'}>{xlmRate(quote.fx.centavos_per_xlm)}</b>
                                </div>
                            )}
                            {/* The 50% rule, as three rows: what the engine
                                requires of the borrower, what they've said
                                they'll carry, and what that leaves for the
                                guarantors. */}
                            {product === 'guarantor' && productQuote.cover_required !== null && (
                                <div className='lending-quote-row'>
                                    <span>Your share ({params.policy.borrower_cover_min_pct}% minimum)</span>
                                    <b>{pesos(productQuote.cover_required)}</b>
                                </div>
                            )}
                            {product === 'guarantor' && coverTotal > 0 && (
                                <div className='lending-quote-row'>
                                    <span>You're covering</span>
                                    <b className={coverShort ? 'is-warn' : 'is-good'}>{pesos(coverTotal)}</b>
                                </div>
                            )}
                            {product === 'guarantor' && guarantorGap !== null && !coverLeavesNoGap && (
                                <div className='lending-quote-row'>
                                    <span>Pledges needed</span>
                                    <b>{pesos(guarantorGap)}</b>
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
