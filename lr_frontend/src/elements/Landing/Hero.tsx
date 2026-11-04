import { useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import { MoveDown } from 'lucide-react'
import LoanProducts from './LoanProducts'

const DOCS_URL = 'https://kentice.gitbook.io/lendrow'

function Hero() {
    const [scrolled, setScrolled] = useState(false)

    useEffect(() => {
        const onScroll = () => setScrolled(window.scrollY > 24)
        onScroll()
        window.addEventListener('scroll', onScroll, { passive: true })
        return () => window.removeEventListener('scroll', onScroll)
    }, [])

    return (
        <section className='hero landing-columns' aria-labelledby='hero-title'>
            <div className='hero-content'>
                <h1 className='hero-content__title' id='hero-title'>
                    <span>Capital</span> that moves at your <span>pace.</span>
                </h1>
                <p className='hero-content__subtitle'>
                    KYC, lending, borrowing, repayment, and portfolio insight, unified in one elegant workspace for borrowers and lenders alike.
                </p>
                <div className='hero-content__actions'>
                    <a className='landing-button hero-content__cta' href={DOCS_URL} target='_blank' rel='noreferrer'>Read Docs</a>
                    <Link className='landing-button landing-button--secondary hero-content__records' to='/records'>Loan records</Link>
                </div>
                <a className={`hero-scroll hero-scroll--mobile${scrolled ? ' hero-scroll--hidden' : ''}`} href='#loan-products'>
                    <MoveDown size={12} strokeWidth={1.3} aria-hidden='true' />
                    <span>Scroll down to see more</span>
                </a>
            </div>
            <LoanProducts />
            <a className={`hero-scroll hero-scroll--desktop${scrolled ? ' hero-scroll--hidden' : ''}`} href='#verification'>
                <MoveDown size={12} strokeWidth={1.3} aria-hidden='true' />
                <span>Scroll down to see more</span>
            </a>
        </section>
    )
}

export default Hero
