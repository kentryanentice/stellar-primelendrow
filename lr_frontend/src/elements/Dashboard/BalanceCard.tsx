import { useNavigate } from 'react-router-dom'
import { pesos } from '../../functions/Lending/money'
import type { PoolResponse } from '../../functions/Lending/types'

/**
 * The money rail's headline, without the rail: what can be withdrawn, what's
 * locked, and a way to the Lend page where deposits and withdrawals actually
 * happen. No deposit/withdraw form here on purpose — ManageFundsCard pulls
 * the PayPal bootstrap, and the dashboard should paint without it.
 */
function BalanceCard({ data }: { data: PoolResponse }) {
    const navigate = useNavigate()
    const { me } = data
    const locked = me.lent + me.collateral + me.pledged

    return (
        <section className='lending-card'>
            <span className='lending-stat-label'>Your balance</span>
            <div className='dash-balance-figure'>
                <span className='dash-balance-value'>{pesos(me.available)}</span>
                <span className='lending-muted'>withdrawable</span>
            </div>
            <div className='dash-rows'>
                <div><span>Locked in loans</span><b>{pesos(locked)}</b></div>
                <div><span>In the pool</span><b>{pesos(me.available + locked)}</b></div>
            </div>
            <div className='dash-actions'>
                <button type='button' className='lending-btn-primary' onClick={() => navigate('/lending')}>Deposit</button>
                <button type='button' className='lending-btn' onClick={() => navigate('/lending')}>Withdraw</button>
            </div>
        </section>
    )
}

export default BalanceCard
