import type { KYCState } from './types'
import { truncateAddress } from '../../functions/Wallet/wallet'

type ReviewStepProps = Pick<KYCState,
    'firstName' | 'middleName' | 'lastName' | 'idNumber' | 'dob' | 'matched' | 'matchScore' | 'livenessVerified' | 'walletAddress' | 'goToStep'
>

export default function ReviewStep({ firstName, middleName, lastName, idNumber, dob, matched, matchScore, livenessVerified, walletAddress, goToStep }: ReviewStepProps) {
    const rows: [string, string][] = [
        ['First name', firstName],
        ['Middle name', middleName],
        ['Last name', lastName],
        ['ID number', idNumber],
        ['Date of birth', dob],
        ['Face match', matched ? `Matched ${matchScore}%` : 'Not yet verified'],
        ['Liveness', livenessVerified ? 'Confirmed' : 'Needs manual review'],
        ['Stellar wallet', walletAddress ? `Connected — ${truncateAddress(walletAddress)}` : 'Not connected'],
    ]

    return (
        <div className='kyc-review'>
            <div className='kyc-review-head'>
                <p>Information review</p>
                <button type='button' onClick={() => goToStep(2)}>Edit</button>
            </div>
            <div className='kyc-review-rows'>
                {rows.map(([label, value]) => (
                    <div key={label} className='kyc-review-row'>
                        <span>{label}</span>
                        <span>{value || '—'}</span>
                    </div>
                ))}
            </div>
        </div>
    )
}
