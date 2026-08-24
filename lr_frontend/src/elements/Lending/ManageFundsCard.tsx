import { useState } from 'react'
import { Lock } from 'lucide-react'
import useMyFunds from '../../functions/Lending/useMyFunds'
import { parsePesoInput, pesos, pesosCompact } from '../../functions/Lending/money'
import type { PoolResponse } from '../../functions/Lending/types'
import PayPalButton from './PayPalButton'

type Tab = 'deposit' | 'withdraw'

/**
 * The sidebar money rail: the caller's balance at a glance (withdrawable /
 * locked / total deposited), then one active form at a time — a Deposit tab
 * and a Withdraw tab, rather than both forms shown together — so there's
 * always exactly one clear action in front of the user. The lot-by-lot
 * breakdown lives in its own card in the main column (YourDepositsCard);
 * this card only answers "what can I do with my money right now."
 */
function ManageFundsCard({ data, onChanged }: { data: PoolResponse; onChanged: () => void }) {
    const { me, params } = data
    const { confirmDeposit, confirming, withdraw, withdrawing } = useMyFunds(onChanged)

    const [tab, setTab] = useState<Tab>('deposit')
    const [depositInput, setDepositInput] = useState('')
    const [withdrawInput, setWithdrawInput] = useState('')

    const depositCentavos = parsePesoInput(depositInput)
    const withdrawCentavos = parsePesoInput(withdrawInput)
    const depositTooSmall = depositCentavos !== null && depositCentavos < params.policy.min_deposit
    const withdrawTooBig = withdrawCentavos !== null && withdrawCentavos > me.available

    const locked = me.lent + me.collateral + me.pledged
    const totalDeposited = me.available + locked

    return (
        <section className='lending-card lending-card-funds'>
            <div className='lending-rail-balance'>
                <span className='lending-stat-label'>Your balance</span>
                <div className='lending-rail-balance-row'>
                    <span className='lending-rail-balance-value'>{pesos(me.available)}</span>
                    <span className='lending-muted'>withdrawable</span>
                </div>
            </div>

            <div className='lending-rail-secondary'>
                <div>
                    <span className='lending-muted'>Locked in loans</span>
                    <span>{pesos(locked)}</span>
                </div>
                <div>
                    <span className='lending-muted'>Total deposited</span>
                    <span>{pesos(totalDeposited)}</span>
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

            <div className='lending-tab-group'>
                <button type='button' className={`lending-tab${tab === 'deposit' ? ' is-active' : ''}`} onClick={() => setTab('deposit')}>
                    Deposit
                </button>
                <button type='button' className={`lending-tab${tab === 'withdraw' ? ' is-active' : ''}`} onClick={() => setTab('withdraw')}>
                    Withdraw
                </button>
            </div>

            {tab === 'deposit' ? (
                <div className='lending-rail-form'>
                    <div className='lending-rail-input'>
                        <span>₱</span>
                        <input
                            inputMode='decimal'
                            placeholder='0.00'
                            value={depositInput}
                            onChange={e => setDepositInput(e.target.value)}
                            disabled={confirming}
                            aria-label='Deposit amount'
                        />
                    </div>
                    {depositTooSmall ? (
                        <p className='lending-field-error'>Minimum deposit is {pesos(params.policy.min_deposit)}.</p>
                    ) : (
                        <p className='lending-muted'>Minimum {pesos(params.policy.min_deposit)} · funds are lent out as borrowers request</p>
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
            ) : (
                <div className='lending-rail-form'>
                    <div className='lending-rail-input'>
                        <span>₱</span>
                        <input
                            inputMode='decimal'
                            placeholder='0.00'
                            value={withdrawInput}
                            onChange={e => setWithdrawInput(e.target.value)}
                            disabled={withdrawing}
                            aria-label='Withdraw amount'
                        />
                        <button
                            type='button'
                            className='lending-rail-max'
                            disabled={withdrawing || me.available <= 0}
                            onClick={() => setWithdrawInput((me.available / 100).toFixed(2))}
                        >
                            Max
                        </button>
                    </div>
                    {withdrawTooBig ? (
                        <p className='lending-field-error'>Only {pesos(me.available)} of your deposit is withdrawable right now.</p>
                    ) : (
                        <p className='lending-muted'>Up to {pesos(me.available)} available now</p>
                    )}
                    <button
                        type='button'
                        className='lending-btn-primary'
                        disabled={!withdrawCentavos || withdrawTooBig || withdrawing}
                        onClick={async () => {
                            if (!withdrawCentavos) return
                            if (await withdraw(withdrawCentavos)) setWithdrawInput('')
                        }}
                    >
                        {withdrawing ? 'Withdrawing…' : 'Withdraw to wallet'}
                    </button>
                </div>
            )}
        </section>
    )
}

export default ManageFundsCard
