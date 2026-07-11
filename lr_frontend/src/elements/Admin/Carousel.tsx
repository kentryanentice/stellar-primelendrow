import { useCallback, useEffect, useRef, useState } from 'react'
import { CircleCheckBig, ChevronLeft, ChevronRight } from 'lucide-react'
import type { AdminState } from './types'
import Card from './Card'

type CarouselProps = Pick<AdminState,
    'queue' | 'page' | 'totalPages' | 'queueLoading' | 'queueLoadingMore' | 'queueError' | 'loadMore' | 'openReview'
>

export default function Carousel({
    queue, page, totalPages, queueLoading, queueLoadingMore, queueError, loadMore, openReview,
}: CarouselProps) {
    const trackRef = useRef<HTMLDivElement>(null)
    const firstIdRef = useRef<string | null>(null)
    const [atStart, setAtStart] = useState(true)
    const [atEnd, setAtEnd] = useState(false)
    const [activeGroup, setActiveGroup] = useState(0)

    // decides whether to pull in the next backend batch using the value it
    // just measured, not React state — reacting to the `atEnd` state in a
    // separate effect meant it could fire on a stale default (atEnd starts
    // true-ish before any real measurement), cascading through every page
    // on load instead of waiting for an actual scroll to the end
    const updateEdges = useCallback(() => {
        const track = trackRef.current
        if (!track) return
        const nowAtEnd = track.scrollLeft + track.clientWidth >= track.scrollWidth - 1
        setAtStart(track.scrollLeft <= 1)
        setAtEnd(nowAtEnd)

        const card = track.firstElementChild as HTMLElement | null
        const step = card ? card.offsetWidth + parseFloat(getComputedStyle(track).columnGap || '0') : 0
        setActiveGroup(step > 0 ? Math.floor(Math.round(track.scrollLeft / step) / 3) : 0)

        if (nowAtEnd && !queueLoadingMore && page < totalPages) loadMore()
    }, [queueLoadingMore, page, totalPages, loadMore])

    // only snap back to the first card when the queue was actually replaced
    // (fresh load / refresh) — an appended batch (loadMore) keeps the same
    // first item, so scrolling stays right where the admin left it
    useEffect(() => {
        const newFirstId = queue[0]?.id ?? null
        if (newFirstId !== firstIdRef.current) {
            trackRef.current?.scrollTo({ left: 0 })
            firstIdRef.current = newFirstId
        }
        updateEdges()
    }, [queue, updateEdges])

    const stepCard = (dir: 1 | -1) => {
        const track = trackRef.current
        const card = track?.firstElementChild as HTMLElement | null
        if (!track || !card) return
        const gap = parseFloat(getComputedStyle(track).columnGap || '0')
        track.scrollBy({ left: dir * (card.offsetWidth + gap), behavior: 'smooth' })
    }

    const goToGroup = (i: number) => {
        const card = trackRef.current?.children[i * 3] as HTMLElement | undefined
        card?.scrollIntoView({ behavior: 'smooth', inline: 'start', block: 'nearest' })
    }

    if (queueLoading) return (
        <div className='admin-status-block'>
            <span className='admin-spinner' aria-hidden='true' />
            <p className='admin-muted'>Loading queue…</p>
        </div>
    )
    if (queueError) return <p className='admin-muted admin-status-block'>Couldn’t load the queue. Please try again.</p>

    if (queue.length === 0) {
        return (
            <div className='admin-empty'>
                <div className='admin-empty-icon'><CircleCheckBig aria-hidden='true' /></div>
                <p className='admin-empty-title'>All caught up</p>
                <p className='admin-muted'>Every submission in the queue has been reviewed. New applicants will appear here.</p>
            </div>
        )
    }

    return (
        <>
            <div className='admin-carousel'>
                <button
                    type='button'
                    className='admin-carousel-arrow'
                    aria-label='Previous card'
                    disabled={atStart}
                    onClick={() => stepCard(-1)}
                >
                    <ChevronLeft />
                </button>

                <div className='admin-carousel-track' ref={trackRef} onScroll={updateEdges}>
                    {queue.map(item => (
                        <Card key={item.id} item={item} onOpen={openReview} />
                    ))}
                </div>

                <button
                    type='button'
                    className='admin-carousel-arrow'
                    aria-label='Next card'
                    disabled={atEnd}
                    onClick={() => stepCard(1)}
                >
                    <ChevronRight />
                </button>
            </div>

            {queue.length > 3 && (
                <div className='admin-page-dots'>
                    {Array.from({ length: Math.ceil(queue.length / 3) }, (_, i) => (
                        <button
                            key={i}
                            type='button'
                            className={`admin-page-dot${activeGroup === i ? ' is-active' : ''}`}
                            aria-label={`Go to cards ${i * 3 + 1}–${Math.min(i * 3 + 3, queue.length)}`}
                            aria-current={activeGroup === i}
                            onClick={() => goToGroup(i)}
                        />
                    ))}
                </div>
            )}
        </>
    )
}
