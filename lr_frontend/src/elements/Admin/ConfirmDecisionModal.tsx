import { useEffect } from 'react'
import { CheckCircle, XCircle } from 'lucide-react'
import type { AdminState } from './types'
import { REJECT_PRESETS } from '../../functions/Admin/AdminFunctions'

type ConfirmDecisionModalProps = Pick<AdminState,
    'detail' | 'deciding' | 'decide' |
    'confirmingApprove' | 'closeApproveConfirm' |
    'rejecting' | 'closeRejectConfirm' | 'reason' | 'setReason'
>

/**
 * Gates both decisions behind an explicit confirm step, in one shared modal:
 * approving or rejecting a KYC submission isn't reversible from here, so a
 * misclick on the review panel's buttons (which just *open* this now, rather
 * than deciding immediately) can still be backed out of.
 */
export default function ConfirmDecisionModal({
    detail, deciding, decide,
    confirmingApprove, closeApproveConfirm,
    rejecting, closeRejectConfirm, reason, setReason,
}: ConfirmDecisionModalProps) {
    const open = confirmingApprove || rejecting

    // lock the page behind the modal, same as the image lightbox
    useEffect(() => {
        if (!open) return
        const prev = document.body.style.overflow
        document.body.style.overflow = 'hidden'
        return () => { document.body.style.overflow = prev }
    }, [open])

    if (!open || !detail) return null

    const close = confirmingApprove ? closeApproveConfirm : closeRejectConfirm
    const name = `${detail.first_name} ${detail.last_name}`.trim() || 'This applicant'

    return (
        <div className='admin-confirm-overlay' onClick={deciding ? undefined : close}>
            <div
                className='admin-confirm-modal'
                role='dialog'
                aria-modal='true'
                aria-label={confirmingApprove ? 'Confirm approval' : 'Confirm rejection'}
                onClick={e => e.stopPropagation()}
            >
                {confirmingApprove ? (
                    <>
                        <h3>Approve this submission?</h3>
                        <p className='admin-muted'>
                            {name} will be marked verified and notified right away. Make sure the photo comparison and details actually check out first.
                        </p>
                        <div className='admin-actions-row'>
                            <button type='button' className='admin-btn-ghost' disabled={deciding} onClick={close}>
                                Cancel
                            </button>
                            <button type='button' className='admin-btn-approve' disabled={deciding} onClick={() => decide('approve')}>
                                <CheckCircle /> {deciding ? 'Approving…' : 'Confirm approval'}
                            </button>
                        </div>
                    </>
                ) : (
                    <div className='admin-reject-form'>
                        <h3>Reject this submission?</h3>
                        <p className='admin-muted'>Select a reason for rejecting this submission. This is recorded and sent to the applicant.</p>
                        <div className='admin-reject-presets'>
                            {REJECT_PRESETS.map(preset => (
                                <button
                                    key={preset}
                                    type='button'
                                    className={`admin-reject-preset${reason === preset ? ' is-active' : ''}`}
                                    onClick={() => setReason(preset)}
                                >
                                    {preset}
                                </button>
                            ))}
                        </div>
                        <textarea
                            value={reason}
                            onChange={e => setReason(e.target.value)}
                            placeholder='Add a note explaining the rejection…'
                            maxLength={500}
                        />
                        <div className='admin-actions-row'>
                            <button type='button' className='admin-btn-ghost' disabled={deciding} onClick={close}>
                                Cancel
                            </button>
                            <button
                                type='button'
                                className='admin-btn-reject'
                                disabled={deciding || !reason.trim()}
                                onClick={() => decide('reject')}
                            >
                                <XCircle /> {deciding ? 'Rejecting…' : 'Confirm rejection'}
                            </button>
                        </div>
                    </div>
                )}
            </div>
        </div>
    )
}
