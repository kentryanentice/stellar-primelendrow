import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom'
import { seoConfigTypes } from '../types/SEOTypes'

import ThemeProvider from './ThemeProvider'
import SEOProvider from './SEOProvider'
import { SessionProvider } from './SessionProvider'
import AccessProvider from './AccessProvider'
import { ToastProvider } from './ToastProvider'
import Theme from '../elements/Theme'
import Landing from '../pages/Landing'
import Auth from '../pages/Auth'
import Dashboard from '../pages/Dashboard'

function RoutesProvider() {
    return (
        <ThemeProvider>
            <ToastProvider>
                <BrowserRouter>
                    <SessionProvider>
                        <AccessProvider>
                            <Routes>
                                <Route path='/' element={<> <Theme /> <SEOProvider {...seoConfigTypes} /> <Landing /> </>} />
                                <Route path='/auth' element={<> <SEOProvider {...seoConfigTypes} /> <Auth /> </>} />
                                <Route path='/dashboard' element={<> <SEOProvider {...seoConfigTypes} /> <Dashboard /> </>} />
                                <Route path='*' element={<Navigate to='/auth' replace />} />
                            </Routes>
                        </AccessProvider>
                    </SessionProvider>
                </BrowserRouter>
            </ToastProvider>
        </ThemeProvider>
    )
}

export default RoutesProvider
