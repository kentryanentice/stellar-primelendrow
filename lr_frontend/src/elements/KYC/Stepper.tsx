import { IdCard, Scan, Camera, Wallet, ListCheck, Check, type LucideIcon } from 'lucide-react'
import type { KYCState } from './types'
import type { KYCStep } from '../../functions/KYC/KYCFunctions'

const STEPS: { key: KYCStep; label: string; sub: string; icon: LucideIcon }[] = [
    { key: 1, label: 'ID Upload', sub: 'Upload or capture your ID', icon: IdCard },
    { key: 2, label: 'ID Scan', sub: 'Auto-fill from your ID', icon: Scan },
    { key: 3, label: 'Selfie', sub: 'Match your face to ID', icon: Camera },
    { key: 4, label: 'Wallet', sub: 'Connect a Stellar wallet', icon: Wallet },
    { key: 5, label: 'Review', sub: 'Confirm and submit', icon: ListCheck },
]

type StepperProps = Pick<KYCState, 'step' | 'maxStep' | 'goToStep'>

export default function Stepper({ step, maxStep, goToStep }: StepperProps) {
    return (
        <aside className='kyc-stepper'>
            <div className='kyc-stepper-line' />
            <div className='kyc-stepper-list'>
                {STEPS.map(s => {
                    const isDone = s.key < step
                    const isActive = s.key === step
                    const clickable = s.key <= maxStep
                    const Icon = s.icon
                    return (
                        <button
                            key={s.key}
                            type='button'
                            disabled={!clickable}
                            onClick={() => goToStep(s.key)}
                            className={`kyc-step${isActive ? ' is-active' : ''}${isDone ? ' is-done' : ''}`}
                        >
                            <span className='kyc-step-circle'>
                                {isDone ? <Check /> : <Icon />}
                            </span>
                            <span className='kyc-step-text'>
                                <span className='kyc-step-label'>{s.label}</span>
                                <span className='kyc-step-sub'>{s.sub}</span>
                            </span>
                        </button>
                    )
                })}
            </div>
        </aside>
    )
}
