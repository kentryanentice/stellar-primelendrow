import { useCallback, useEffect, useRef } from 'react'
import { Users, User } from 'lucide-react'
import type { AdminState } from './types'
import { idTypeLabel, scoreVerdict } from '../../functions/Admin/AdminFunctions'

type RailProps = Pick<AdminState,
    'queue' | 'total' | 'page' | 'totalPages' | 'queueLoadingMore' | 'selectedId' | 'openReview' | 'loadMore'
>

/** The compact list of pending submissions shown alongside the detail panel
 *  in review mode — the same queue data the carousel uses, just a narrower
 *  row per item so switching between submissions doesn't require going back
 *  to the carousel. Mirrors the carousel's scroll-to-edge pagination (see
 *  Carousel's updateEdges) so the rail isn't stuck showing only page 1. */
export default function Rail({ queue, total, page, totalPages, queueLoadingMore, selectedId, openReview, loadMore }: RailProps) {
    const listRef = useRef<HTMLDivElement>(null)

    const checkEnd = useCallback(() => {
        const list = listRef.current
        if (!list) return
        const atEnd = list.scrollTop + list.clientHeight >= list.scrollHeight - 1
        if (atEnd && !queueLoadingMore && page < totalPages) loadMore()
    }, [queueLoadingMore, page, totalPages, loadMore])

    // covers the case where the loaded rows don't even fill the list's
    // visible height — there's nothing to scroll, so onScroll below would
    // never fire, and the rail would silently cap at whatever page loaded first
    useEffect(() => { checkEnd() }, [checkEnd, queue])

    return (
        <div className='admin-rail'>
            <div className='admin-rail-head'>
                <Users />
                <span>Pending ({total})</span>
            </div>
            <div className='admin-rail-list' ref={listRef} onScroll={checkEnd}>
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
                {queueLoadingMore && (
                    <div className='admin-rail-loading-more'>
                        <span className='admin-spinner' aria-hidden='true' />
                    </div>
                )}
            </div>
        </div>
    )
}
