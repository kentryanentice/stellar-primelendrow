import { useEffect, useRef, useState, type KeyboardEvent, type MouseEvent } from 'react'

const TABS = ['docs', 'terms', 'privacy'] as const
type Tab = typeof TABS[number]
type DocumentState = { tab: Tab; anchor?: string }

export function useLandingDocuments() {
    const [document, setDocument] = useState<DocumentState | null>(null)
    const [closing, setClosing] = useState(false)
    const [more, setMore] = useState(false)
    const returnFocus = useRef<HTMLElement | null>(null)
    const closeTimer = useRef(0)
    const open = document !== null
    const tab = document?.tab ?? 'docs'

    const updateEdge = () => {
        const body = window.document.getElementById('plr-dlg-body')
        setMore(Boolean(body && body.scrollTop + body.clientHeight < body.scrollHeight - 4))
    }

    useEffect(() => {
        if (!open) return
        const page = window.document.documentElement
        const overflow = page.style.overflow
        page.style.overflow = 'hidden'
        return () => {
            page.style.overflow = overflow
            returnFocus.current?.focus({ preventScroll: true })
        }
    }, [open])

    useEffect(() => {
        if (!document) return
        const frame = requestAnimationFrame(() => {
            const body = window.document.getElementById('plr-dlg-body')
            const anchor = document.anchor ? window.document.getElementById(document.anchor) : null
            if (body) body.scrollTop = anchor ? Math.max(0, anchor.offsetTop - body.offsetTop - 16) : 0
            window.document.getElementById(`plr-tab-${document.tab}`)?.focus({ preventScroll: true })
            updateEdge()
        })
        return () => cancelAnimationFrame(frame)
    }, [document])

    useEffect(() => () => window.clearTimeout(closeTimer.current), [])

    const show = (nextTab: Tab, anchor?: string) => (event: MouseEvent<HTMLAnchorElement>) => {
        event.preventDefault()
        window.clearTimeout(closeTimer.current)
        if (!open) returnFocus.current = event.currentTarget
        setClosing(false)
        setDocument({ tab: nextTab, anchor })
    }

    const close = () => {
        if (!open || closing) return
        const finish = () => {
            setDocument(null)
            setClosing(false)
            setMore(false)
        }
        if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) finish()
        else {
            setClosing(true)
            closeTimer.current = window.setTimeout(finish, 220)
        }
    }

    const select = (nextTab: Tab) => {
        if (nextTab !== tab) setDocument({ tab: nextTab })
    }

    const onKey = (event: KeyboardEvent<HTMLDivElement>) => {
        if (event.key === 'Escape') {
            event.stopPropagation()
            close()
            return
        }
        if ((event.target as HTMLElement).getAttribute('role') === 'tab') {
            const index = TABS.indexOf(tab)
            const next = event.key === 'ArrowRight' ? (index + 1) % TABS.length
                : event.key === 'ArrowLeft' ? (index + TABS.length - 1) % TABS.length
                : event.key === 'Home' ? 0 : event.key === 'End' ? TABS.length - 1 : -1
            if (next >= 0) {
                event.preventDefault()
                select(TABS[next])
                return
            }
        }
        if (event.key !== 'Tab') return
        const focusable = Array.from(event.currentTarget.querySelectorAll<HTMLElement>(
            'button:not([disabled]):not([tabindex="-1"]), a[href], [tabindex="0"]',
        )).filter(element => element.getClientRects().length > 0)
        const first = focusable[0]
        const last = focusable.at(-1)
        const active = window.document.activeElement
        if (event.shiftKey && active === first) {
            event.preventDefault()
            last?.focus()
        } else if (!event.shiftKey && active === last) {
            event.preventDefault()
            first?.focus()
        }
    }

    return {
        dlgOpen: open,
        dlgCls: closing ? 'is-closing' : '',
        dlgBodyCls: more ? 'is-more' : '',
        dlgIs: { docs: tab === 'docs', terms: tab === 'terms', privacy: tab === 'privacy' },
        dlgSel: { docs: tab === 'docs', terms: tab === 'terms', privacy: tab === 'privacy' },
        dlgTi: { docs: tab === 'docs' ? 0 : -1, terms: tab === 'terms' ? 0 : -1, privacy: tab === 'privacy' ? 0 : -1 },
        dlgTabCls: { docs: tab === 'docs' ? 'is-active' : '', terms: tab === 'terms' ? 'is-active' : '', privacy: tab === 'privacy' ? 'is-active' : '' },
        openDocs: show('docs'), openDocsInterest: show('docs', 'dlg-doc-interest'),
        openTerms: show('terms'), openPrivacy: show('privacy'),
        onTabDocs: () => select('docs'), onTabTerms: () => select('terms'), onTabPrivacy: () => select('privacy'),
        closeDlg: close,
        onDlgOverlay: (event: MouseEvent<HTMLDivElement>) => { if (event.target === event.currentTarget) close() },
        onDlgKey: onKey,
        onDlgScroll: updateEdge,
    }
}
