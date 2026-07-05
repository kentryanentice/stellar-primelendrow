import { Check, CheckCircle } from 'lucide-react'
import type { KYCState } from './types'

type SuccessScreenProps = Pick<KYCState, 'matchScore'> & { onDone: () => void }

export default function SuccessScreen({ matchScore, onDone }: SuccessScreenProps) {
    return (
        <div className='kyc-success'>
            <div className='kyc-success-icon'><Check /></div>
            <h2>You're verified!</h2>
            <p>Your identity has been confirmed. You can now access your account.</p>
            <div className='kyc-success-badges'>
                <span><CheckCircle />ID verified</span>
                <span><CheckCircle />Selfie {matchScore}% match</span>
                <span><CheckCircle />Wallet connected</span>
                <span><CheckCircle />Payment complete</span>
            </div>
            <button type='button' className='kyc-btn-primary' onClick={onDone}>Go to dashboard</button>
        </div>
    )
}
