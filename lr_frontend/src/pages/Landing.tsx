import Hero from "../elements/Landing/Hero"
import KycShowcase from "../elements/Landing/KycShowcase"

function Landing() {
    return (
        <main className='landing'>
            <div className='landing-bg' aria-hidden='true' />
            <Hero />
            <KycShowcase />
        </main>
    )
}

export default Landing
