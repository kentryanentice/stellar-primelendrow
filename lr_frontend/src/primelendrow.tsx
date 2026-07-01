import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { HelmetProvider } from 'react-helmet-async'
import './scss/primelendrow.scss'

import RoutesProvider from './providers/RoutesProvider'

const container = document.getElementById('primelendrow')

if (!container) {
    throw new Error('Root element #primelendrow not found in DOM')
}

const root = createRoot(container)

root.render(

    <StrictMode>

        <HelmetProvider>

            <RoutesProvider />

        </HelmetProvider>

    </StrictMode>

)