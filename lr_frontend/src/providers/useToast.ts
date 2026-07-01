import { createContext, useContext } from 'react'

export type ToastApi = {
    success: (message: string) => void
    error: (message: string) => void
    info: (message: string) => void
    dismiss: (id: number) => void
}

export const ToastContext = createContext<ToastApi | null>(null)

export function useToast() {
    const toast = useContext(ToastContext)
    if (!toast) throw new Error('useToast must be used inside ToastProvider')
    return toast
}
