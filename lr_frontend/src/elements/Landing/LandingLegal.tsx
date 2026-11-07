import { useEffect, useRef } from 'react'
import { legalContent } from './legalContent'

type Props = { kind: 'terms' | 'privacy'; onClose: () => void }

export default function LandingLegal({ kind, onClose }: Props) {
    const dialog = useRef<HTMLDialogElement>(null)
    const content = legalContent[kind]

    useEffect(() => {
        const element = dialog.current
        const previousFocus = document.activeElement as HTMLElement | null
        const previousOverflow = document.body.style.overflow
        element?.showModal()
        document.body.style.overflow = 'hidden'
        return () => {
            element?.close()
            document.body.style.overflow = previousOverflow
            previousFocus?.focus({ preventScroll: true })
        }
    }, [])

    return (
        <dialog className='landing-legal' ref={dialog} aria-labelledby='landing-legal-title'
            onCancel={onClose} onClick={event => { if (event.target === event.currentTarget) onClose() }}>
            <div className='landing-legal__header'>
                <h2 id='landing-legal-title'>{content.title}</h2>
                <button type='button' onClick={onClose} aria-label='Close' autoFocus>×</button>
            </div>
            <div className='landing-legal__body'>
                {content.sections.map(section => (
                    <section key={section.title}>
                        <h3>{section.title}</h3>
                        {section.paragraphs.map(paragraph => <p key={paragraph}>{paragraph}</p>)}
                    </section>
                ))}
            </div>
        </dialog>
    )
}
