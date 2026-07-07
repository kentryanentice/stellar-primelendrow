import { Clock, CheckCircle } from 'lucide-react'
import type { KYCState } from './types'

type SuccessScreenProps = Pick<KYCState, 'matchScore'> & { onDone: () => void }

export default function SuccessScreen({ matchScore, onDone }: SuccessScreenProps) {
    return (
        <div className='kyc-success'>
            <div className='kyc-success-icon'><Clock /></div>
            <h2>Verification submitted</h2>
            <p>Thanks! We’ve received your details and our team is now reviewing your identity. Your account will be approved once the review is complete — you can track the status anytime under Settings.</p>
            <div className='kyc-success-badges'>
                <span><CheckCircle />ID submitted</span>
                <span><CheckCircle />Selfie {matchScore}% match</span>
                <span><CheckCircle />Wallet connected</span>
            </div>
            <button type='button' className='kyc-btn-primary' onClick={onDone}>Go to dashboard</button>
        </div>
    )
}
