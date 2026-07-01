import { Link, useLocation } from 'react-router-dom'
import { useTheme } from '../providers/ThemeProvider'

function Header() {
    const { theme, toggleTheme } = useTheme()
    const { pathname } = useLocation()
    const logoSrc = '/pictures/primelendrow.webp'
    const themeButton = (
        <button className="theme-icon" type="button" aria-label="Toggle theme" onClick={toggleTheme}>
            <i className={theme === 'light' ? 'bx bx-sun' : 'bx bxs-sun'} />
        </button>
    )

    if (pathname === '/auth') {
        return <header className="site-header site-header--auth">{themeButton}</header>
    }

    const isLanding = pathname === '/'

    return (
        <header className={`site-header${isLanding ? ' site-header--landing' : ''}`}>
            <Link to="/" className="site-header__brand">
                <img src={logoSrc} alt="" />
                Prime<span>LendRow</span>
            </Link>
            <div className="site-header__actions">
                <Link to="/auth" className="site-header__cta">Get Started</Link>
                {themeButton}
            </div>
        </header>
    )
}

export default Header
