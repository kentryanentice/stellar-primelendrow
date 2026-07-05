import { createContext, useContext, useEffect, useState } from 'react'
import type { ReactNode } from 'react'

type Accent = 'blue' | 'green'

type AccentContextType = {
    accent: Accent
    toggleAccent: () => void
    setAccent: (a: Accent) => void
}

const AccentContext = createContext<AccentContextType | null>(null)

function AccentProvider({ children }: { children: ReactNode }) {
    const [accent, setAccent] = useState<Accent>(() => {
        if (typeof window !== 'undefined') {
            const stored = localStorage.getItem('accent') as Accent | null
            if (stored === 'blue' || stored === 'green') return stored
        }
        return 'blue'
    })

    useEffect(() => {
        document.documentElement.setAttribute('data-accent', accent)
        localStorage.setItem('accent', accent)
    }, [accent])

    const toggleAccent = () => setAccent(prev => (prev === 'blue' ? 'green' : 'blue'))

    return (
        <AccentContext.Provider value={{ accent, toggleAccent, setAccent }}>
            {children}
        </AccentContext.Provider>
    )
}

// eslint-disable-next-line react-refresh/only-export-components
export function useAccent() {
    const ctx = useContext(AccentContext)
    if (!ctx) throw new Error('useAccent must be used within AccentProvider')
    return ctx
}

export default AccentProvider
