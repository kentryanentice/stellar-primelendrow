import { Info, CheckCircle, ShieldQuestion, Flag, User } from 'lucide-react'
import { idTypeLabel, timeAgo, scoreVerdict, isFlagged, type PendingItem } from '../../functions/Admin/AdminFunctions'

type CardProps = {
    item: PendingItem
    onOpen: (id: string) => void
}

export default function Card({ item, onOpen }: CardProps) {
    const verdict = scoreVerdict(item.face_match_score)
    const pct = item.face_match_score ?? 0

    return (
        <div className='admin-card'>
            <div className='admin-card-head'>
                <span className='admin-card-type'>
                    {idTypeLabel(item.id_type)}
                    {isFlagged(item.face_match_score) && <Flag className='admin-card-flag' aria-label='Flagged for review' />}
                </span>
                <span className='admin-card-time'>{timeAgo(item.created_at)}</span>
            </div>

            <div className='admin-card-photos'>
                <div className='admin-card-photo'>
                    {item.selfie_image_url
                        ? <img src={item.selfie_image_url} alt='Live selfie' />
                        : <User className='admin-card-photo-placeholder' aria-hidden='true' />}
                    <span className='admin-card-photo-tag is-live'>LIVE</span>
                </div>
                <div className='admin-card-photo'>
                    {item.id_image_url
                        ? <img src={item.id_image_url} alt='Submitted ID' />
                        : <User className='admin-card-photo-placeholder' aria-hidden='true' />}
                    <span className='admin-card-photo-tag'>ID</span>
                </div>
            </div>

            <div className='admin-card-body'>
                <div className='admin-card-score-row'>
                    <div className='admin-card-score'>
                        <span className={`admin-card-score-value ${verdict.cls}`}>{item.face_match_score != null ? `${item.face_match_score}%` : '—'}</span>
                        <span className='admin-card-score-label'>face match</span>
                    </div>
                    <span className={`admin-card-verdict ${verdict.cls}`}>{verdict.label}</span>
                </div>
                <div className='admin-card-track'>
                    <div className={`admin-card-fill ${verdict.cls}`} style={{ width: `${pct}%` }} />
                </div>

                <div className='admin-card-badges'>
                    <span className={`admin-card-badge${item.liveness_passed ? ' is-passed' : ' is-bypassed'}`}>
                        {item.liveness_passed ? <CheckCircle /> : <ShieldQuestion />}
                        Liveness {item.liveness_passed ? 'passed' : 'bypassed'}
                    </span>
                </div>

                <button type='button' className='admin-card-info' onClick={() => onOpen(item.id)}>
                    <Info /> View Info
                </button>
            </div>
        </div>
    )
}
