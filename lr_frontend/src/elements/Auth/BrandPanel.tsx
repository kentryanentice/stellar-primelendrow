import { Link } from 'react-router-dom'
import { ShieldIcon, StarIcon } from './icons'

const LOGO = '/pictures/lr.png'
const PROFILE = '/pictures/profile.png'

interface BrandPanelProps {
    busy: boolean
}

export default function BrandPanel({ busy }: BrandPanelProps) {
    return (
        <div className='auth-brand'>
            <img className='auth-brand-bg' src='/pictures/hexagon.png' alt='' />

            <div className='auth-brand-inner'>
                <div className='auth-brand-top'>
                    <Link
                        to='/'
                        className={`auth-brand-mark${busy ? ' is-disabled' : ''}`}
                        aria-disabled={busy}
                        tabIndex={busy ? -1 : undefined}
                        onClick={e => { if (busy) e.preventDefault() }}
                    >
                        <img src={LOGO} alt='PrimeLendRow' />
                    </Link>
                    <span>Prime<span className='auth-brand-accent'>LendRow</span></span>
                </div>

                <div className='auth-brand-head'>
                    <h2>Capital that moves at your pace.</h2>
                    <p>KYC, lending, borrowing, repayment, and portfolio insight, unified in one elegant workspace.</p>
                </div>

                <div>
                    <div className='auth-quote'>
                        <div className='auth-stars'>{Array.from({ length: 5 }, (_, i) => <StarIcon key={i} />)}</div>
                        <p>"."</p>
                        <div className='auth-quote-author'>
                            <img className='auth-avatar' src={PROFILE} alt='Kent' />
                            <div>
                                <div className='auth-author-name'>K</div>
                                <div className='auth-author-role'>CTO, PrimeLendRow</div>
                            </div>
                        </div>
                    </div>
                    <div className='auth-brand-foot'><ShieldIcon /> Bank-grade encryption · SOC 2 Type II</div>
                </div>
            </div>
        </div>
    )
}
