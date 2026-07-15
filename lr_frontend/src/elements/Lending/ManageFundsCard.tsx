import { useState } from 'react'
import { PiggyBank, Lock, ArrowDownToLine, ArrowUpFromLine } from 'lucide-react'
import useMyFunds from '../../functions/Lending/useMyFunds'
import { parsePesoInput, pesos, pesosCompact } from '../../functions/Lending/money'
import type { PoolResponse } from '../../functions/Lending/types'
import PayPalButton from './PayPalButton'

/**
 * The caller's money in the pool: withdrawable vs. locked, deposit via
 * PayPal, and withdraw of whatever is still wearing the 'available' badge.
 * The lot-by-lot breakdown lives next to this in its own card
 * (YourDepositsCard) — split out so each card answers one question.
 */
function ManageFundsCard({ data, onChanged }: { data: PoolResponse; onChanged: () => void }) {
    const { me, params } = data
    const { confirmDeposit, confirming, withdraw, withdrawing } = useMyFunds(onChanged)

    const [depositInput, setDepositInput] = useState('')
    const [withdrawInput, setWithdrawInput] = useState('')

    const depositCentavos = parsePesoInput(depositInput)
    const withdrawCentavos = parsePesoInput(withdrawInput)
    const depositTooSmall = depositCentavos !== null && depositCentavos < params.policy.min_deposit
    const withdrawTooBig = withdrawCentavos !== null && withdrawCentavos > me.available

    const locked = me.lent + me.collateral + me.pledged

    return (
        <section className='lending-card lending-card-funds'>
            <div className='lending-card-head'>
                <span className='lending-card-icon is-accent'><PiggyBank /></span>
                <h2>Manage funds</h2>
            </div>

            <div className='lending-funds-grid'>
                <div className='lending-funds-tile is-good'>
                    <span className='lending-stat-label'>Withdrawable</span>
                    <span className='lending-stat-value is-good'>{pesos(me.available)}</span>
                </div>
                <div className='lending-funds-tile'>
                    <span className='lending-stat-label'>Locked</span>
                    <span className='lending-stat-value'>{pesos(locked)}</span>
                </div>
            </div>

            {locked > 0 && (
                <p className='lending-muted lending-locked-breakdown'>
                    <Lock />
                    {me.lent > 0 && <>Funding loans {pesosCompact(me.lent)}</>}
                    {me.lent > 0 && (me.collateral > 0 || me.pledged > 0) && ' · '}
                    {me.collateral > 0 && <>Backing my loan {pesosCompact(me.collateral)}</>}
                    {me.collateral > 0 && me.pledged > 0 && ' · '}
                    {me.pledged > 0 && <>Pledged {pesosCompact(me.pledged)}</>}
                </p>
            )}

            <div className='lending-funds-actions'>
                <div className='lending-action'>
                    <label className='lending-label' htmlFor='lending-deposit-amount'>
                        <ArrowDownToLine /> Deposit to pool
                    </label>
                    <input
                        id='lending-deposit-amount'
                        className='lending-input'
                        inputMode='decimal'
                        placeholder={`Min ${pesos(params.policy.min_deposit)}`}
                        value={depositInput}
                        onChange={e => setDepositInput(e.target.value)}
                        disabled={confirming}
                    />
                    {depositTooSmall ? (
                        <p className='lending-field-error'>Minimum deposit is {pesos(params.policy.min_deposit)}.</p>
                    ) : (
                        <p className='lending-muted'>Added to the community pool</p>
                    )}
                    <PayPalButton
                        amountCentavos={depositTooSmall ? null : depositCentavos}
                        description='PrimeLendRow pool deposit'
                        onApproved={orderId => {
                            setDepositInput('')
                            return confirmDeposit(orderId)
                        }}
                    />
                    {confirming && <p className='lending-muted'>Confirming your deposit…</p>}
                </div>

                <div className='lending-action'>
                    <label className='lending-label' htmlFor='lending-withdraw-amount'>
                        <ArrowUpFromLine /> Withdraw
                    </label>
                    <input
                        id='lending-withdraw-amount'
                        className='lending-input'
                        inputMode='decimal'
                        placeholder={`Up to ${pesos(me.available)}`}
                        value={withdrawInput}
                        onChange={e => setWithdrawInput(e.target.value)}
                        disabled={withdrawing}
                    />
                    {withdrawTooBig ? (
                        <p className='lending-field-error'>Only {pesos(me.available)} of your deposit is withdrawable right now.</p>
                    ) : (
                        <p className='lending-muted'>Up to {pesos(me.available)} available now</p>
                    )}
                    <button
                        type='button'
                        className='lending-btn'
                        disabled={!withdrawCentavos || withdrawTooBig || withdrawing}
                        onClick={async () => {
                            if (!withdrawCentavos) return
                            if (await withdraw(withdrawCentavos)) setWithdrawInput('')
                        }}
                    >
                        {withdrawing ? 'Withdrawing…' : 'Withdraw'}
                    </button>
                </div>
            </div>
        </section>
    )
}

export default ManageFundsCard
