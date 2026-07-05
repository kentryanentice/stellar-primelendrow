import {
    useCallback,
    useEffect,
    useLayoutEffect,
    useMemo,
    useRef,
    useState,
    type ReactNode,
} from 'react'
import { CheckCircle, AlertCircle, Info, X } from 'lucide-react'
import { ToastContext, type ToastApi } from './useToast'

type ToastKind = 'success' | 'error' | 'info'

type Toast = {
    id: number
    kind: ToastKind
    message: string
    leaving?: boolean
}

const EXIT_DURATION = 200
const TOAST_GAP = 10
const FALLBACK_HEIGHT = 54

export function ToastProvider({ children }: { children: ReactNode }) {
    const [toasts, setToasts] = useState<Toast[]>([])
    const [heights, setHeights] = useState<Record<number, number>>({})
    const nextId = useRef(1)
    const timers = useRef<Map<number, number>>(new Map())
    const lastToast = useRef<{ kind: ToastKind; message: string; at: number } | null>(null)
    const rowRefs = useRef<Map<number, HTMLDivElement>>(new Map())

    // measure real toast heights so offsets below are exact, not guessed
    useLayoutEffect(() => {
        const observer = new ResizeObserver(entries => {
            setHeights(prev => {
                const next = { ...prev }
                for (const entry of entries) {
                    const id = Number((entry.target as HTMLElement).dataset.toastId)
                    next[id] = entry.contentRect.height
                }
                return next
            })
        })
        rowRefs.current.forEach(el => observer.observe(el))
        return () => observer.disconnect()
    }, [toasts])

    // stack toasts via translateY instead of normal flow, so a dismissed
    // toast's neighbors slide into place on the compositor (no layout thrash)
    let cursor = 0
    const offsets = new Map<number, number>()
    for (const toast of toasts) {
        offsets.set(toast.id, cursor)
        if (!toast.leaving) cursor += (heights[toast.id] ?? FALLBACK_HEIGHT) + TOAST_GAP
    }

    const dismiss = useCallback((id: number) => {
        const timer = timers.current.get(id)
        if (timer) window.clearTimeout(timer)
        timers.current.delete(id)
        setToasts(current => current.map(toast => (toast.id === id ? { ...toast, leaving: true } : toast)))
        window.setTimeout(() => {
            setToasts(current => current.filter(toast => toast.id !== id))
        }, EXIT_DURATION)
    }, [])

    const show = useCallback((kind: ToastKind, rawMessage: string) => {
        const message = rawMessage.trim()
        if (!message) return

        const now = Date.now()
        if (
            lastToast.current
            && lastToast.current.kind === kind
            && lastToast.current.message === message
            && now - lastToast.current.at < 1500
        ) {
            return
        }
        lastToast.current = { kind, message, at: now }

        const id = nextId.current
        nextId.current += 1

        setToasts(current => [...current.slice(-3), { id, kind, message }])
        const duration = kind === 'error' ? 7000 : 4500
        const timer = window.setTimeout(() => dismiss(id), duration)
        timers.current.set(id, timer)
    }, [dismiss])

    useEffect(() => () => {
        timers.current.forEach(timer => window.clearTimeout(timer))
        timers.current.clear()
    }, [])

    const api = useMemo<ToastApi>(() => ({
        success: message => show('success', message),
        error: message => show('error', message),
        info: message => show('info', message),
        dismiss,
    }), [dismiss, show])

    return (
        <ToastContext.Provider value={api}>
            {children}
            <div className='toast-stack' role='status' aria-live='polite' aria-relevant='additions text'>
                {toasts.map(toast => (
                    <div
                        key={toast.id}
                        ref={el => { if (el) rowRefs.current.set(toast.id, el); else rowRefs.current.delete(toast.id) }}
                        data-toast-id={toast.id}
                        className={`toast-row${toast.leaving ? ' is-leaving' : ''}`}
                        style={{ transform: `translateY(${offsets.get(toast.id) ?? 0}px)` }}
                    >
                        <div className={`toast toast--${toast.kind}`}>
                            {toast.kind === 'success' ? <CheckCircle aria-hidden='true' />
                                : toast.kind === 'error' ? <AlertCircle aria-hidden='true' />
                                    : <Info aria-hidden='true' />}
                            <p>{toast.message}</p>
                            <button type='button' aria-label='Dismiss notification' onClick={() => dismiss(toast.id)}>
                                <X aria-hidden='true' />
                            </button>
                        </div>
                    </div>
                ))}
            </div>
        </ToastContext.Provider>
    )
}
