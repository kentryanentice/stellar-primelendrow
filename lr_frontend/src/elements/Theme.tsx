import { useEffect, useState } from 'react'
import { Link, useLocation } from 'react-router-dom'
import { Bubbles, Leaf } from 'lucide-react'
import { useAccent } from '../providers/AccentProvider'

function Header() {
    const { accent, toggleAccent } = useAccent()
    const { pathname } = useLocation()
    const isLanding = pathname === '/'
    const [scrolled, setScrolled] = useState(false)

    useEffect(() => {
        if (!isLanding) return

        const onScroll = () => setScrolled(window.scrollY > 24)
        onScroll()
        window.addEventListener('scroll', onScroll, { passive: true })
        return () => window.removeEventListener('scroll', onScroll)
    }, [isLanding])

    const logoSrc = '/pictures/primelendrow.webp'
    const accentButton = (
        <button className="theme-icon" type="button" aria-label="Toggle accent color" aria-pressed={accent === 'green'} onClick={toggleAccent}>
            {accent === 'blue' ? <Bubbles aria-hidden="true" /> : <Leaf aria-hidden="true" />}
        </button>
    )

    return (
        <header className={`site-header${isLanding ? ' site-header--landing' : ''}${isLanding && scrolled ? ' site-header--scrolled' : ''}`}>
            <Link to="/" className="site-header__brand" aria-label="PrimeLendRow home">
                <img src={logoSrc} alt="" />
                <span className="site-header__brand-text">Prime<span>LendRow</span></span>
            </Link>
            <nav className="site-header__actions" aria-label="Primary">
                <Link to="/records" className="site-header__link">Loan records</Link>
                <Link to="/auth" className="site-header__cta">Get Started</Link>
                {accentButton}
            </nav>
        </header>
    )
}

export default Header
