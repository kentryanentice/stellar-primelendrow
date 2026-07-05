import Hero from "../elements/Landing/Hero"
import KycShowcase from "../elements/Landing/KycShowcase"

// the .landing shell owns the shared backdrop (gradient + hexagon texture);
// each section lives in its own module under elements/Landing — add the next
// one after <KycShowcase /> with its scss partial in scss/pages/landing/
function Landing() {
    return (
        <main className='landing'>
            <img className='landing-bg' src='/pictures/hero-bg.png' alt='' />
            <Hero />
            <KycShowcase />
        </main>
    )
}

export default Landing
