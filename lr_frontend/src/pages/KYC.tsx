import { useNavigate } from 'react-router-dom'
import useKYCFunctions from '../functions/KYC/KYCFunctions'
import Stepper from '../elements/KYC/Stepper'
import IdUploadStep from '../elements/KYC/IdUploadStep'
import IdScanStep from '../elements/KYC/IdScanStep'
import SelfieStep from '../elements/KYC/SelfieStep'
import WalletStep from '../elements/KYC/WalletStep'
import ReviewStep from '../elements/KYC/ReviewStep'
import SuccessScreen from '../elements/KYC/SuccessScreen'

const STEP_COPY: Record<number, [string, string]> = {
    1: ['Upload your ID', 'Take a clear photo of your government-issued ID, or upload one from your device.'],
    2: ['Scanning your ID', 'We’ll pull your details automatically — double check them before continuing.'],
    3: ['Take a selfie', 'We’ll compare it to the photo on your ID to confirm it’s really you.'],
    4: ['Connect your wallet', 'Link a Stellar wallet to manage funds.'],
    5: ['Review your information', 'Make sure everything looks right before you submit.'],
}

function KYC() {
    const navigate = useNavigate()
    const kyc = useKYCFunctions()
    const { step, canContinue, advance, back, finish, scanning, scanned, scanId, comparing, matched, compareFaces, submitting } = kyc

    if (step === 6) {
        return (
            <main className='kyc'>
                <SuccessScreen matchScore={kyc.matchScore} onDone={() => navigate('/dashboard')} />
            </main>
        )
    }

    const [title, subtitle] = STEP_COPY[step]

    let primaryLabel = 'Continue'
    let primaryAction = advance
    let primaryDisabled = !canContinue

    if (step === 2 && !scanned) {
        primaryLabel = scanning ? 'Scanning…' : 'Scan ID'
        primaryAction = scanId
        primaryDisabled = scanning || !kyc.idImageUrl
    } else if (step === 3 && !matched) {
        primaryLabel = comparing ? 'Comparing…' : 'Compare faces'
        primaryAction = compareFaces
        primaryDisabled = comparing || !kyc.selfieImageUrl
    } else if (step === 5) {
        primaryLabel = submitting ? 'Submitting…' : 'Submit'
        primaryAction = finish
        primaryDisabled = submitting
    }

    return (
        <main className='kyc'>
            <div className='kyc-wrap'>
                <h1 className='kyc-title'>Identity Verification</h1>
                <p className='kyc-lede'>Verify your identity and get started</p>

                <div className='kyc-layout'>
                    <Stepper step={kyc.step} maxStep={kyc.maxStep} goToStep={kyc.goToStep} />

                    <section className='kyc-main'>
                        <p className='kyc-step-count'>Step {step}/5</p>
                        <h2 className='kyc-step-title'>{title}</h2>
                        <p className='kyc-step-subtitle'>{subtitle}</p>

                        <div className='kyc-card'>
                            {step === 1 && <IdUploadStep {...kyc} />}
                            {step === 2 && <IdScanStep {...kyc} />}
                            {step === 3 && <SelfieStep {...kyc} />}
                            {step === 4 && <WalletStep {...kyc} />}
                            {step === 5 && <ReviewStep {...kyc} />}
                        </div>

                        <div className='kyc-actions'>
                            {step > 1 && <button type='button' className='kyc-btn-outline' onClick={back}>Back</button>}
                            <button type='button' className='kyc-btn-primary' disabled={primaryDisabled} onClick={primaryAction}>
                                {primaryLabel}
                            </button>
                        </div>
                    </section>
                </div>
            </div>
        </main>
    )
}

export default KYC
