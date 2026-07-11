import { X, CheckCircle, XCircle, User, Maximize2 } from 'lucide-react'
import type { AdminState } from './types'
import { idTypeLabel, formatDate, scoreVerdict, REJECT_PRESETS } from '../../functions/Admin/AdminFunctions'
import { truncateAddress } from '../../functions/Wallet/wallet'
import ImageLightbox from './ImageLightbox'

type ReviewPanelProps = Pick<AdminState,
    'detail' | 'detailLoading' | 'detailError' | 'closeReview' |
    'lightboxOpen' | 'openLightbox' | 'closeLightbox' |
    'rejecting' | 'setRejecting' | 'reason' | 'setReason' | 'deciding' | 'decide'
>

export default function ReviewPanel({
    detail, detailLoading, detailError, closeReview,
    lightboxOpen, openLightbox, closeLightbox,
    rejecting, setRejecting, reason, setReason, deciding, decide,
}: ReviewPanelProps) {
    return (
        <div className='admin-panel'>
            {detailLoading ? (
                <div className='admin-status-block'>
                    <span className='admin-spinner' aria-hidden='true' />
                    <p className='admin-muted'>Loading submission…</p>
                </div>
            ) : detailError || !detail ? (
                <p className='admin-muted admin-status-block'>Couldn’t load this submission. Please try again.</p>
            ) : (
                <>
                    <div className='admin-panel-head'>
                        <div className='admin-panel-head-text'>
                            <span className='admin-panel-eyebrow'>Submission review</span>
                            <div className='admin-panel-name-row'>
                                <h2>{detail.first_name} {detail.middle_name ? `${detail.middle_name} ` : ''}{detail.last_name}</h2>
                                <span className='admin-panel-type'>{idTypeLabel(detail.id_type)}</span>
                            </div>
                        </div>
                        <button type='button' className='admin-panel-close' aria-label='Back to queue' onClick={closeReview}>
                            <X />
                        </button>
                        <div className='admin-panel-accent-line' />
                    </div>

                    <div className='admin-panel-scroll'>
                        {/* comparison row: live selfie, score ring, submitted ID */}
                        <div className='admin-compare'>
                            <button type='button' className='admin-compare-photo' onClick={openLightbox}>
                                {detail.selfie_image_url
                                    ? <img src={detail.selfie_image_url} alt='Live selfie' />
                                    : <div className='admin-compare-placeholder'><User aria-hidden='true' /></div>}
                                <span className='admin-compare-tag is-live'>LIVE SELFIE</span>
                                <span className='admin-compare-expand'><Maximize2 aria-hidden='true' /></span>
                                <span className='admin-compare-caption'>Captured on device</span>
                            </button>

                            <div className='admin-compare-ring-wrap'>
                                {(() => {
                                    const verdict = scoreVerdict(detail.face_match_score)
                                    const pct = detail.face_match_score ?? 0
                                    return (
                                        <>
                                            <div
                                                className={`admin-compare-ring ${verdict.cls}`}
                                                style={{ background: `conic-gradient(var(--ring-color) ${pct}%, rgba(255,255,255,.09) ${pct}%)` }}
                                            >
                                                <div className='admin-compare-ring-inner'>
                                                    <span className='admin-compare-ring-value'>{detail.face_match_score != null ? `${detail.face_match_score}%` : '—'}</span>
                                                    <span className='admin-compare-ring-label'>face match</span>
                                                </div>
                                            </div>
                                            <span className={`admin-verdict ${verdict.cls}`}>{verdict.label}</span>
                                        </>
                                    )
                                })()}
                            </div>

                            <div className='admin-compare-id'>
                                <span className='admin-compare-id-tag'>Submitted ID</span>
                                <div className='admin-compare-id-head'>
                                    <span className='admin-compare-id-dot' />
                                    <div>
                                        <p className='admin-compare-id-eyebrow'>Government-issued ID</p>
                                        <p className='admin-compare-id-type'>{idTypeLabel(detail.id_type)}</p>
                                    </div>
                                </div>
                                <div className='admin-compare-id-body'>
                                    <button type='button' className='admin-compare-id-photo' onClick={openLightbox}>
                                        {detail.id_image_url
                                            ? <img src={detail.id_image_url} alt='Submitted ID portrait' />
                                            : <User aria-hidden='true' />}
                                        <span className='admin-compare-expand'><Maximize2 aria-hidden='true' /></span>
                                    </button>
                                    <div className='admin-compare-id-fields'>
                                        <div><span>Surname</span><p>{detail.last_name}</p></div>
                                        <div><span>Given names</span><p>{detail.first_name} {detail.middle_name ?? ''}</p></div>
                                        <div><span>ID number</span><p>{detail.id_number}</p></div>
                                    </div>
                                </div>
                            </div>
                        </div>

                        {/* signal tiles — only the two signals we actually compute */}
                        <div className='admin-signals'>
                            <div className='admin-signal'>
                                <div className='admin-signal-head'>
                                    <span>Face match</span>
                                    <span className={`admin-signal-dot ${scoreVerdict(detail.face_match_score).cls}`} />
                                </div>
                                <p className={`admin-signal-value ${scoreVerdict(detail.face_match_score).cls}`}>
                                    {detail.face_match_score != null ? `${detail.face_match_score}%` : '—'}
                                </p>
                            </div>
                            <div className='admin-signal'>
                                <div className='admin-signal-head'>
                                    <span>Liveness</span>
                                    <span className={`admin-signal-dot${detail.liveness_passed ? ' is-strong' : ' is-review'}`} />
                                </div>
                                <p className={`admin-signal-value${detail.liveness_passed ? ' is-strong' : ' is-review'}`}>
                                    {detail.liveness_passed ? 'Passed' : 'Bypassed'}
                                </p>
                            </div>
                        </div>

                        {/* details grid — only fields we actually have on file */}
                        <div className='admin-detail-fields'>
                            <div><dt>Date of birth</dt><dd>{detail.dob}</dd></div>
                            <div><dt>ID number</dt><dd>{detail.id_number}</dd></div>
                            <div><dt>Wallet address</dt><dd title={detail.wallet_address ?? undefined}>{detail.wallet_address ? truncateAddress(detail.wallet_address) : '—'}</dd></div>
                            <div><dt>Submitted</dt><dd>{formatDate(detail.created_at)}</dd></div>
                            {detail.reviewed_at != null && <div><dt>Reviewed</dt><dd>{formatDate(detail.reviewed_at)}</dd></div>}
                        </div>
                    </div>

                    <div className='admin-panel-actions'>
                        {!rejecting ? (
                            <div className='admin-actions-row is-decide'>
                                <button type='button' className='admin-btn-approve' disabled={deciding} onClick={() => decide('approve')}>
                                    <CheckCircle /> Approve
                                </button>
                                <button type='button' className='admin-btn-reject' disabled={deciding} onClick={() => setRejecting(true)}>
                                    <XCircle /> Reject
                                </button>
                            </div>
                        ) : (
                            <div className='admin-reject-form'>
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
                                    <button
                                        type='button'
                                        className='admin-btn-ghost'
                                        disabled={deciding}
                                        onClick={() => { setRejecting(false); setReason('') }}
                                    >
                                        Cancel
                                    </button>
                                    <button
                                        type='button'
                                        className='admin-btn-reject'
                                        disabled={deciding || !reason.trim()}
                                        onClick={() => decide('reject')}
                                    >
                                        <XCircle /> Confirm rejection
                                    </button>
                                </div>
                            </div>
                        )}
                    </div>

                    <ImageLightbox detail={detail} lightboxOpen={lightboxOpen} closeLightbox={closeLightbox} />
                </>
            )}
        </div>
    )
}
