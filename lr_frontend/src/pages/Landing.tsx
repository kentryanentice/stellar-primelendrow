import { useSyncExternalStore } from 'react'
import LandingDesktop from '../elements/Landing/LandingDesktop'
import LandingMobile from '../elements/Landing/LandingMobile'
import { useLandingPresentation } from '../elements/Landing/useLandingPresentation'

const MOBILE_QUERY = '(max-width: 720px)'
const subscribe = (onChange: () => void) => {
    const media = window.matchMedia(MOBILE_QUERY)
    media.addEventListener('change', onChange)
    return () => media.removeEventListener('change', onChange)
}

function LandingExperience({ mobile }: { mobile: boolean }) {
    const { root, v } = useLandingPresentation(mobile)
    return mobile ? <LandingMobile root={root} v={v} /> : <LandingDesktop root={root} v={v} />
}

export default function Landing() {
    const mobile = useSyncExternalStore(subscribe, () => window.matchMedia(MOBILE_QUERY).matches, () => false)
    return <LandingExperience key={mobile ? 'mobile' : 'desktop'} mobile={mobile} />
}
