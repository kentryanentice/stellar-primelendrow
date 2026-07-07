import { Link, useLocation } from 'react-router-dom'
import { Bubbles, Leaf } from 'lucide-react'
import { useAccent } from '../providers/AccentProvider'

function Header() {
    const { accent, toggleAccent } = useAccent()
    const { pathname } = useLocation()
    const logoSrc = '/pictures/primelendrow.webp'
    const accentButton = (
        <button className="theme-icon" type="button" aria-label="Toggle accent color" onClick={toggleAccent}>
            {accent === 'blue' ? <Bubbles /> : <Leaf />}
        </button>
    )

    const isLanding = pathname === '/'

    return (
        <header className={`site-header${isLanding ? ' site-header--landing' : ''}`}>
            <Link to="/" className="site-header__brand">
                <img src={logoSrc} alt="PrimeLendRow" />
                <span className="site-header__brand-text">Prime<span>LendRow</span></span>
            </Link>
            <div className="site-header__actions">
                <Link to="/auth" className="site-header__cta">Get Started</Link>
                {accentButton}
            </div>
        </header>
    )
}

export default Header
