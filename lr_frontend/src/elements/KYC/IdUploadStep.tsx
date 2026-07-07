import { CloudUpload, Camera } from 'lucide-react'
import type { KYCState } from './types'

type IdUploadStepProps = Pick<KYCState,
    | 'idImageUrl' | 'idSource' | 'idFileName' | 'handleIdFile' | 'resetId'
    | 'cameraTarget' | 'videoRef' | 'openCamera' | 'closeCamera' | 'captureId'
>

export default function IdUploadStep({
    idImageUrl, idSource, idFileName, handleIdFile, resetId,
    cameraTarget, videoRef, openCamera, closeCamera, captureId,
}: IdUploadStepProps) {
    if (cameraTarget === 'id') {
        return (
            <div className='kyc-camera'>
                <video ref={videoRef} autoPlay playsInline muted className='kyc-camera-video' />
                <div className='kyc-camera-actions'>
                    <button type='button' className='kyc-btn-ghost' onClick={closeCamera}>Cancel</button>
                    <button type='button' className='kyc-btn-primary' onClick={captureId}>Capture</button>
                </div>
            </div>
        )
    }

    if (idImageUrl) {
        return (
            <div className='kyc-preview-row'>
                <div className='kyc-preview-thumb'>
                    <img src={idImageUrl} alt='Your ID' />
                </div>
                <div>
                    <p className='kyc-preview-title'>ID captured</p>
                    <p className='kyc-preview-sub'>{idSource === 'upload' ? idFileName : 'Captured with camera'}</p>
                    <button type='button' className='kyc-btn-outline' onClick={resetId}>Retake / choose another</button>
                </div>
            </div>
        )
    }

    return (
        <div className='kyc-dropzone-grid'>
            <label className='kyc-dropzone'>
                <CloudUpload />
                <span className='kyc-dropzone-title'>Upload an ID</span>
                <span className='kyc-dropzone-sub'>Drag &amp; drop or click to browse</span>
                <input type='file' accept='image/*' onChange={handleIdFile} hidden />
            </label>
            <button type='button' className='kyc-dropzone' onClick={() => openCamera('id')}>
                <Camera />
                <span className='kyc-dropzone-title'>Take a photo of an ID</span>
                <span className='kyc-dropzone-sub'>Use your camera to capture your ID</span>
            </button>
        </div>
    )
}
