import { CheckCircle, Camera, ShieldCheck } from 'lucide-react'
import type { KYCState } from './types'
import { LIVENESS_PASS_PERCENT, type LivenessStatus } from '../../functions/KYC/liveness'

const LIVENESS_COPY: Record<Exclude<LivenessStatus, 'idle'>, string> = {
    loading: 'Loading liveness check…',
    'no-face': 'Position your face in the frame',
    'position-face': 'Fit your face inside the outline',
    'awaiting-blink': 'Blink to confirm you’re really there',
    'spoof-warning': 'We can’t confirm a live camera feed — make sure you’re not holding up a photo or screen',
    passed: 'Liveness confirmed — hold still…',
    bypassed: 'Liveness check unavailable — frame your face and capture manually',
    timeout: 'We couldn’t auto-confirm liveness — frame your face and capture manually. Your submission will be reviewed.',
}

type SelfieStepProps = Pick<KYCState,
    | 'selfieImageUrl' | 'resetSelfie'
    | 'idImageUrl' | 'idFaceUrl' | 'comparing' | 'faceModelsLoading' | 'matched' | 'matchScore'
    | 'livenessStatus' | 'livenessPassed' | 'liveScore'
    | 'cameraTarget' | 'videoRef' | 'openCamera' | 'closeCamera' | 'captureSelfie'
>

export default function SelfieStep({
    selfieImageUrl, resetSelfie,
    idImageUrl, idFaceUrl, comparing, faceModelsLoading, matched, matchScore,
    livenessStatus, livenessPassed, liveScore,
    cameraTarget, videoRef, openCamera, closeCamera, captureSelfie,
}: SelfieStepProps) {
    if (cameraTarget === 'selfie') {
        return (
            <div className='kyc-camera'>
                <div className='kyc-camera-frame'>
                    <video ref={videoRef} autoPlay playsInline muted className='kyc-camera-video' />
                    <div className={
                        'kyc-camera-guide'
                        + (livenessStatus === 'passed' ? ' kyc-camera-guide--ok' : '')
                        + (livenessStatus === 'spoof-warning' ? ' kyc-camera-guide--warn' : '')
                    } />
                </div>
                {livenessStatus !== 'bypassed' && livenessStatus !== 'timeout' && (
                    <div className='kyc-live-meter'>
                        <div className='kyc-live-meter-label'>
                            <span>Live face</span>
                            <span className='kyc-live-meter-value'>{liveScore}%</span>
                        </div>
                        <div className='kyc-live-meter-track'>
                            <div
                                className={'kyc-live-meter-fill' + (liveScore >= LIVENESS_PASS_PERCENT ? ' kyc-live-meter-fill--ok' : '')}
                                style={{ width: `${liveScore}%` }}
                            />
                            <div className='kyc-live-meter-target' style={{ left: `${LIVENESS_PASS_PERCENT}%` }} />
                        </div>
                    </div>
                )}
                {livenessStatus !== 'idle' && (
                    <p className={
                        'kyc-liveness'
                        + (livenessStatus === 'passed' ? ' kyc-liveness--ok' : '')
                        + (livenessStatus === 'spoof-warning' ? ' kyc-liveness--warn' : '')
                    }>
                        {livenessStatus === 'passed' && <ShieldCheck />}
                        {LIVENESS_COPY[livenessStatus]}
                    </p>
                )}
                <div className='kyc-camera-actions'>
                    <button type='button' className='kyc-btn-ghost' onClick={closeCamera}>Cancel</button>
                    {(livenessStatus === 'bypassed' || livenessStatus === 'timeout') && (
                        <button type='button' className='kyc-btn-primary' disabled={!livenessPassed} onClick={captureSelfie}>Capture</button>
                    )}
                </div>
            </div>
        )
    }

    if (matched) {
        return (
            <div className='kyc-preview-row'>
                <div className='kyc-compare-pair'>
                    <figure className='kyc-compare-item'>
                        <div className='kyc-preview-thumb kyc-preview-thumb--sm'>
                            {(idFaceUrl ?? idImageUrl) && <img src={idFaceUrl ?? idImageUrl ?? undefined} alt='Photo on your ID' />}
                        </div>
                        <figcaption>ID photo</figcaption>
                    </figure>
                    <figure className='kyc-compare-item'>
                        <div className='kyc-preview-thumb kyc-preview-thumb--sm'>
                            {selfieImageUrl && <img src={selfieImageUrl} alt='Your selfie' />}
                        </div>
                        <figcaption>Selfie</figcaption>
                    </figure>
                </div>
                <div>
                    <div className='kyc-match-badge'>
                        <CheckCircle />
                        <span>Match confirmed</span>
                    </div>
                    <p className='kyc-preview-sub'>{matchScore}% match to your ID photo</p>
                    <button type='button' className='kyc-btn-outline' onClick={() => { resetSelfie(); openCamera('selfie') }}>Retake selfie</button>
                </div>
            </div>
        )
    }

    if (selfieImageUrl) {
        return (
            <div className='kyc-preview-row'>
                <div className='kyc-preview-thumb kyc-preview-thumb--portrait'><img src={selfieImageUrl} alt='Your selfie' /></div>
                <div>
                    <p className='kyc-preview-title'>
                        {faceModelsLoading ? 'Loading face model…' : comparing ? 'Comparing your photos…' : 'Selfie captured'}
                    </p>
                    <p className='kyc-preview-sub'>Liveness confirmed — ready to compare against your ID photo.</p>
                    {matchScore !== null && !comparing && (
                        <p className='kyc-preview-sub kyc-preview-sub--warn'>{matchScore}% match — not a strong enough match, please try again</p>
                    )}
                    <button type='button' className='kyc-btn-outline' onClick={() => { resetSelfie(); openCamera('selfie') }}>Retake</button>
                </div>
            </div>
        )
    }

    return (
        <button type='button' className='kyc-dropzone' onClick={() => openCamera('selfie')}>
            <Camera />
            <span className='kyc-dropzone-title'>Take a live selfie</span>
            <span className='kyc-dropzone-sub'>Uses your camera with a quick blink check — uploading a photo isn&apos;t allowed for this step</span>
        </button>
    )
}
