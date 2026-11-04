import { useEffect, useRef } from 'react'
import { Link } from 'react-router-dom'
import { IdCard, Scan, Camera, Wallet, ListCheck, type LucideIcon } from 'lucide-react'
import { useSurfaceTilt } from './useSurfaceTilt'

type KycStep = {
    icon: LucideIcon
    step: string
    title: string
    text: string
}

const KYC_STEPS: KycStep[] = [
    { icon: IdCard, step: '01', title: 'Upload your ID', text: 'Submit a government ID and OCR extracts your details instantly.' },
    { icon: Scan, step: '02', title: 'Live document scan', text: 'A quick id scan cross-checks your document in real time.' },
    { icon: Camera, step: '03', title: 'Selfie liveness', text: 'A simple blink check proves it’s really you behind the screen.' },
    { icon: Wallet, step: '04', title: 'Link your wallet', text: 'Connect your Stellar wallet for payouts and repayments.' },
    { icon: ListCheck, step: '05', title: 'Review & submit', text: 'Confirm everything in one summary and you’re verified.' },
]

function KycShowcase() {
    const sectionRef = useRef<HTMLElement>(null)
    useSurfaceTilt(sectionRef, -3, '.kyc-showcase-card')

    useEffect(() => {
        const section = sectionRef.current
        if (!section || !('IntersectionObserver' in window)
            || window.matchMedia('(prefers-reduced-motion: reduce)').matches) return

        section.dataset.animate = 'true'
        const observer = new IntersectionObserver((entries) => {
            entries.forEach((entry) => {
                if (!entry.isIntersecting) return
                const element = entry.target as HTMLElement
                element.dataset.visible = 'true'
                observer.unobserve(element)
            })
        }, { threshold: 0.12, rootMargin: '0px 0px -24px 0px' })

        section.querySelectorAll('[data-kyc-reveal]').forEach((element) => observer.observe(element))
        return () => observer.disconnect()
    }, [])

    return (
        <section id='verification' className='kyc-showcase' aria-labelledby='verification-title' ref={sectionRef}>
            <div className='kyc-showcase-content landing-columns'>
                <div className='kyc-showcase-intro'>
                    <h2 id='verification-title' className='kyc-showcase-intro__title' data-kyc-reveal>
                        Verified in <span>minutes,</span> not <span>days.</span>
                    </h2>
                    <p className='kyc-showcase-intro__subtitle' data-kyc-reveal>
                        Five guided steps pair OCR, live document scanning, and selfie liveness, so lenders trust exactly who they fund, without the paperwork.
                    </p>
                    <Link className='landing-button kyc-showcase-intro__cta' to='/auth' data-kyc-reveal>Start Verification</Link>
                </div>
                <ol className='kyc-showcase-grid' aria-label='Verification steps'>
                    {KYC_STEPS.map(({ icon: Icon, step, title, text }) => (
                        <li className='kyc-showcase-card' key={step} data-kyc-reveal>
                            <span className='kyc-showcase-card__step'>{step}</span>
                            <span className='kyc-showcase-card__icon'>
                                <Icon size={16} strokeWidth={1.6} aria-hidden='true' />
                            </span>
                            <h3 className='kyc-showcase-card__title'>{title}</h3>
                            <p className='kyc-showcase-card__text'>{text}</p>
                        </li>
                    ))}
                </ol>
            </div>
        </section>
    )
}

export default KycShowcase
