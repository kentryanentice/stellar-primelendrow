import { IdCard } from 'lucide-react'
import { ID_TYPE_LABELS } from '../../functions/KYC/idParsing'
import DobPicker from './DobPicker'
import type { KYCState } from './types'

type IdScanStepProps = Pick<KYCState,
    | 'idImageUrl' | 'scanning' | 'scanned' | 'detectedIdType'
    | 'firstName' | 'setFirstName' | 'middleName' | 'setMiddleName' | 'lastName' | 'setLastName'
    | 'idNumber' | 'setIdNumber' | 'dob' | 'setDob'
>

export default function IdScanStep({
    idImageUrl, scanning, scanned, detectedIdType,
    firstName, setFirstName, middleName, setMiddleName, lastName, setLastName,
    idNumber, setIdNumber, dob, setDob,
}: IdScanStepProps) {
    if (!scanned) {
        return (
            <div className='kyc-preview-row'>
                <div className='kyc-preview-thumb kyc-preview-thumb--scan'>
                    {idImageUrl && <img src={idImageUrl} alt='Your ID' />}
                    <div className={`kyc-scan-sweep${scanning ? ' is-active' : ''}`} />
                </div>
                <div>
                    <p className='kyc-preview-title'>{scanning ? 'Scanning your ID…' : 'Ready to scan'}</p>
                    <p className='kyc-preview-sub kyc-preview-sub--wide'>
                        We'll read your ID photo to pre-fill your name, ID number, and date of birth — always double-check the results below before continuing.
                    </p>
                </div>
            </div>
        )
    }

    return (
        <div className='kyc-form-grid'>
            {detectedIdType && (
                <p className='kyc-detected-type kyc-field--wide'>
                    <IdCard />
                    {detectedIdType === 'unknown' ? "Couldn't recognize the ID type" : `Detected: ${ID_TYPE_LABELS[detectedIdType]}`}
                </p>
            )}
            <label className='kyc-field'>
                <span>First name</span>
                <input value={firstName} onChange={e => setFirstName(e.target.value)} />
            </label>
            <label className='kyc-field'>
                <span>Middle name</span>
                <input value={middleName} onChange={e => setMiddleName(e.target.value)} />
            </label>
            <label className='kyc-field'>
                <span>Last name</span>
                <input value={lastName} onChange={e => setLastName(e.target.value)} />
            </label>
            <label className='kyc-field'>
                <span>ID number</span>
                <input value={idNumber} onChange={e => setIdNumber(e.target.value)} />
            </label>
            <label className='kyc-field'>
                <span>Date of birth</span>
                <div className='kyc-dob-input-row'>
                    <input value={dob} onChange={e => setDob(e.target.value)} placeholder='DD/MM/YYYY' />
                    <DobPicker value={dob} onChange={setDob} />
                </div>
            </label>
        </div>
    )
}
