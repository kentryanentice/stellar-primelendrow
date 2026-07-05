import { Link } from 'react-router-dom'
import { IdCard, Scan, Camera, Wallet, ListCheck, type LucideIcon } from 'lucide-react'

type KycStep = {
    icon: LucideIcon
    step: string
    title: string
    text: string
}

const KYC_STEPS: KycStep[] = [
    { icon: IdCard, step: '01', title: 'Upload your ID', text: 'Submit a government ID and OCR extracts your details instantly.' },
    { icon: Scan, step: '02', title: 'Live document scan', text: 'A quick camera scan cross-checks your document in real time.' },
    { icon: Camera, step: '03', title: 'Selfie liveness', text: 'A simple blink check proves it’s really you behind the screen.' },
    { icon: Wallet, step: '04', title: 'Link your wallet', text: 'Connect your Stellar wallet for payouts and repayments.' },
    { icon: ListCheck, step: '05', title: 'Review & submit', text: 'Confirm everything in one summary and you’re verified.' },
]

function KycShowcase() {
    return (
        <section className='kyc-showcase'>
            <div className='kyc-showcase-content'>
                <div className='kyc-showcase-intro'>
                    <h2 className='kyc-showcase-intro__title'>
                        Verified in <span>minutes,</span> not <span>days.</span>
                    </h2>
                    <p className='kyc-showcase-intro__subtitle'>
                        Five guided steps pair OCR, live document scanning, and selfie liveness — so lenders trust exactly who they fund, without the paperwork.
                    </p>
                    <Link className='kyc-showcase-intro__cta' to='/auth'>Start Verification</Link>
                </div>
                <div className='kyc-showcase-grid'>
                    {KYC_STEPS.map(({ icon: Icon, step, title, text }) => (
                        <article className='kyc-showcase-card' key={step}>
                            <span className='kyc-showcase-card__step'>{step}</span>
                            <Icon className='kyc-showcase-card__icon' size={22} strokeWidth={1.6} />
                            <h3 className='kyc-showcase-card__title'>{title}</h3>
                            <p className='kyc-showcase-card__text'>{text}</p>
                        </article>
                    ))}
                </div>
            </div>
        </section>
    )
}

export default KycShowcase
