import { lazy, Suspense } from 'react'
import { BrowserRouter, Routes, Route, Navigate, Outlet } from 'react-router-dom'
import { seoConfigTypes } from '../types/SEOTypes'

import AccentProvider from './AccentProvider'
import SEOProvider from './SEOProvider'
import { SessionProvider } from './SessionProvider'
import AccessProvider from './AccessProvider'
import { ToastProvider } from './ToastProvider'
import Theme from '../elements/Theme'
import AppShell from '../elements/AppShell'
import Landing from '../pages/Landing'
import Auth from '../pages/Auth'
import Dashboard from '../pages/Dashboard'
import Settings from '../pages/Settings'

// OCR/face-match SDKs are ~1.4MB — split out so only /kyc visitors pay for it
const KYC = lazy(() => import('../pages/KYC'))

// shared across every page behind the sidebar so the Sidebar (and its
// collapsed/active state) survives client-side navigation instead of
// unmounting and remounting — which was replaying its mount-in animations
// (flicker) on every page change
function AppShellLayout() {
    return (
        <AppShell>
            <Outlet />
        </AppShell>
    )
}

function RoutesProvider() {
    return (
        <AccentProvider>
            <ToastProvider>
                <BrowserRouter>
                    <SessionProvider>
                        <AccessProvider>
                            <Routes>
                                <Route path='/' element={<> <Theme /> <SEOProvider {...seoConfigTypes} /> <Landing /> </>} />
                                <Route path='/auth' element={<> <SEOProvider {...seoConfigTypes} /> <Auth /> </>} />
                                <Route element={<AppShellLayout />}>
                                    <Route path='/dashboard' element={<> <SEOProvider {...seoConfigTypes} /> <Dashboard /> </>} />
                                    <Route path='/settings' element={<> <SEOProvider {...seoConfigTypes} /> <Settings /> </>} />
                                    <Route path='/verification' element={<> <SEOProvider {...seoConfigTypes} /> <Suspense fallback={<div className='loader' />}><KYC /></Suspense> </>} />
                                </Route>
                                <Route path='*' element={<Navigate to='/auth' replace />} />
                            </Routes>
                        </AccessProvider>
                    </SessionProvider>
                </BrowserRouter>
            </ToastProvider>
        </AccentProvider>
    )
}

export default RoutesProvider
