import { useEffect, useState } from 'react'
import { MoveDown } from 'lucide-react'

const DOCS_URL = 'https://kentice.gitbook.io/lendrow'

function Hero() {
    const [scrolled, setScrolled] = useState(false)

    useEffect(() => {
        const onScroll = () => setScrolled(window.scrollY > 40)
        onScroll()
        window.addEventListener('scroll', onScroll, { passive: true })
        return () => window.removeEventListener('scroll', onScroll)
    }, [])

    return (
        <section className='hero'>
            <div className='hero-content'>
                <h1 className='hero-content__title'>
                    <span>Capital</span> that moves at your <span>pace.</span>
                </h1>
                <p className='hero-content__subtitle'>
                    KYC, lending, borrowing, repayment, and portfolio insight, unified in one elegant workspace for borrowers and lenders alike.
                </p>
                <a className='hero-content__cta' href={DOCS_URL} target='_blank' rel='noreferrer'>Read Docs</a>
            </div>
            <div className={`hero-scroll${scrolled ? ' hero-scroll--hidden' : ''}`}>
                <MoveDown size={15} strokeWidth={1.5} />
                <span>Scroll down to see more</span>
            </div>
        </section>
    )
}

export default Hero
