import { Users, User } from 'lucide-react'
import type { AdminState } from './types'
import { idTypeLabel, scoreVerdict } from '../../functions/Admin/AdminFunctions'

type RailProps = Pick<AdminState, 'queue' | 'total' | 'selectedId' | 'openReview'>

/** The compact list of pending submissions shown alongside the detail panel
 *  in review mode — the same queue data the carousel uses, just a narrower
 *  row per item so switching between submissions doesn't require going back
 *  to the carousel. */
export default function Rail({ queue, total, selectedId, openReview }: RailProps) {
    return (
        <div className='admin-rail'>
            <div className='admin-rail-head'>
                <Users />
                <span>Pending ({total})</span>
            </div>
            <div className='admin-rail-list'>
                {queue.map(item => {
                    const verdict = scoreVerdict(item.face_match_score)
                    return (
                        <button
                            key={item.id}
                            type='button'
                            className={`admin-rail-row${item.id === selectedId ? ' is-active' : ''}`}
                            onClick={() => openReview(item.id)}
                        >
                            <span className='admin-rail-avatar'>
                                {item.selfie_image_url
                                    ? <img src={item.selfie_image_url} alt='' />
                                    : <User aria-hidden='true' />}
                            </span>
                            <span className='admin-rail-info'>
                                <span className='admin-rail-type'>{idTypeLabel(item.id_type)}</span>
                            </span>
                            <span className={`admin-rail-score ${verdict.cls}`}>
                                {item.face_match_score != null ? `${item.face_match_score}%` : '—'}
                            </span>
                        </button>
                    )
                })}
            </div>
        </div>
    )
}
