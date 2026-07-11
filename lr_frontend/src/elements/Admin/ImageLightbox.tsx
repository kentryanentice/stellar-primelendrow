import { useEffect } from 'react'
import { X, User } from 'lucide-react'
import type { AdminState } from './types'

type ImageLightboxProps = Pick<AdminState, 'detail' | 'lightboxOpen' | 'closeLightbox'>

/** Both submitted photos at full size, side by side, so an admin can actually
 *  study facial features against each other instead of squinting at the
 *  review panel's small thumbnails. */
export default function ImageLightbox({ detail, lightboxOpen, closeLightbox }: ImageLightboxProps) {
    // lock the page behind the lightbox so it can't be scrolled while open
    useEffect(() => {
        if (!lightboxOpen) return
        const prev = document.body.style.overflow
        document.body.style.overflow = 'hidden'
        return () => { document.body.style.overflow = prev }
    }, [lightboxOpen])

    if (!lightboxOpen || !detail) return null

    return (
        <div className='admin-lightbox-overlay' onClick={closeLightbox}>
            <div className='admin-lightbox' role='dialog' aria-modal='true' aria-label='Full-size photo comparison' onClick={e => e.stopPropagation()}>
                <button type='button' className='admin-lightbox-close' aria-label='Close' onClick={closeLightbox}>
                    <X />
                </button>
                <div className='admin-lightbox-grid'>
                    <figure className='admin-lightbox-figure'>
                        {detail.selfie_image_url
                            ? <img src={detail.selfie_image_url} alt='Live selfie, full size' />
                            : <div className='admin-lightbox-placeholder'><User aria-hidden='true' /></div>}
                        <figcaption>Live selfie</figcaption>
                    </figure>
                    <figure className='admin-lightbox-figure'>
                        {detail.id_image_url
                            ? <img src={detail.id_image_url} alt='Submitted ID, full size' />
                            : <div className='admin-lightbox-placeholder'><User aria-hidden='true' /></div>}
                        <figcaption>Submitted ID</figcaption>
                    </figure>
                </div>
            </div>
        </div>
    )
}
