import { useEffect, useRef, useState, type MouseEvent } from 'react'
import { useAccent } from '../../providers/AccentProvider'
import { useSurfaceTilt } from './useSurfaceTilt'

type Motion = {
    scrolled: boolean
    footerShown: boolean
    active: string
    seen: Record<string, boolean>
    step: number
    lit: number
    zoomed: boolean
}

const SECTIONS = ['verification', 'lend', 'borrow', 'pay', 'interest', 'faq']

export function useLandingPresentation(mobile: boolean) {
    const root = useRef<HTMLDivElement>(null)
    const navigation = useRef({ locked: false, timer: 0 })
    const { accent, toggleAccent } = useAccent()
    const [legal, setLegal] = useState<'terms' | 'privacy' | null>(null)
    const [faqOpen, setFaqOpen] = useState(1)
    const [motion, setMotion] = useState<Motion>({
        scrolled: false, footerShown: false, active: '', seen: {}, step: 0, lit: 0, zoomed: false,
    })

    useSurfaceTilt(root, -4, '.plr-card-wrap')
    useSurfaceTilt(root, -3, '.plr-step')

    useEffect(() => {
        const element = root.current
        if (!element) return
        const prefix = mobile ? 'plr-m-' : 'plr-'
        const ids = (mobile ? ['products', ...SECTIONS] : SECTIONS).map(id => prefix + id)
        const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)')
        const stacked = window.matchMedia('(max-width: 1100px)')
        const wrap = element.querySelector<HTMLElement>('.plr-vblocks')
        const area = element.querySelector<HTMLElement>('.plr-vstage-area')
        const scene = element.querySelector<HTMLElement>('#plr-verification')
        const footer = element.querySelector<HTMLElement>('.plr-footer')
        const rails = Array.from(element.querySelectorAll<HTMLElement>('.plr-vrail-line, .plr-vrail-dotw'))
        let centers: number[] = []
        let span = 1
        let scrollFrame = 0
        let sceneFrame = 0
        let measureFrame = 0
        let disposed = false
        const current = { rp: 0, z: 0 }
        const target = { rp: 0, z: 0 }

        // Only discrete states render React; the rail and stage stay on the animation frame.
        const update = (patch: Partial<Motion>) => setMotion(previous =>
            Object.entries(patch).some(([key, value]) => previous[key as keyof Motion] !== value)
                ? { ...previous, ...patch } : previous)

        const animateScene = () => {
            if (sceneFrame || disposed) return
            let last = 0
            const tick = (time: number) => {
                const elapsed = last ? Math.min(64, time - last) : 16
                last = time
                const damping = reducedMotion.matches ? 1 : 1 - Math.exp(-elapsed / 120)
                let moving = false
                for (const key of ['rp', 'z'] as const) {
                    const distance = target[key] - current[key]
                    if (Math.abs(distance) > .0004) {
                        current[key] += distance * damping
                        moving = true
                    } else current[key] = target[key]
                }
                rails.forEach(rail => rail.style.setProperty('--rp', current.rp.toFixed(4)))
                area?.style.setProperty('--z', current.z.toFixed(4))
                let lit = 0
                centers.forEach((center, index) => {
                    if (current.rp >= (center - centers[0]) / span - .002) lit = index
                })
                update({ lit, zoomed: current.z > .6 })
                sceneFrame = moving ? requestAnimationFrame(tick) : 0
            }
            sceneFrame = requestAnimationFrame(tick)
        }

        const updateScene = () => {
            if (!wrap || !centers.length) return
            const height = window.innerHeight
            let zoom = 0
            if (!mobile && !stacked.matches && scene) {
                const progress = Math.min(1, Math.max(0, (-scene.getBoundingClientRect().top - height * .06) / (height * .5)))
                zoom = reducedMotion.matches ? Number(progress >= .5) : progress * progress * (3 - 2 * progress)
            }
            const readingLine = ((mobile ? 64 : 58) + height) / 2 - wrap.getBoundingClientRect().top
            const lead = mobile || stacked.matches ? height * .1 : span / 8
            let step = 0
            centers.forEach((center, index) => { if (center <= readingLine + lead) step = index })
            target.rp = Math.min(1, Math.max(0, (readingLine - centers[0]) / span))
            target.z = zoom
            update({ step })
            animateScene()
        }

        const measure = () => {
            if (disposed) return
            const card = area?.querySelector<HTMLElement>('.plr-steps > li')
            const stage = area?.querySelector<HTMLElement>('.plr-vstage')
            if (area && card && stage) {
                let top = 0
                let left = 0
                for (let node: HTMLElement | null = card; node && node !== area; node = node.offsetParent as HTMLElement | null) {
                    top += node.offsetTop
                    left += node.offsetLeft
                }
                area.style.setProperty('--ct', `${top}px`)
                area.style.setProperty('--cl', `${left}px`)
                area.style.setProperty('--cr', `${Math.max(0, stage.offsetWidth - left - card.offsetWidth)}px`)
                area.style.setProperty('--cb', `${Math.max(0, stage.offsetHeight - top - card.offsetHeight)}px`)
            }
            if (wrap) {
                centers = Array.from(wrap.querySelectorAll<HTMLElement>('.plr-vnode')).map(node =>
                    (node.parentElement?.offsetTop ?? 0) + node.offsetTop + node.offsetHeight / 2)
                span = Math.max(1, centers.at(-1)! - centers[0])
                rails.forEach(rail => {
                    rail.style.top = `${centers[0]}px`
                    rail.style.height = `${span}px`
                })
            }
            updateScene()
        }

        const onScroll = () => {
            if (navigation.current.locked) {
                window.clearTimeout(navigation.current.timer)
                navigation.current.timer = window.setTimeout(() => {
                    navigation.current.locked = false
                    onScroll()
                }, 140)
            }
            if (scrollFrame) return
            scrollFrame = requestAnimationFrame(() => {
                scrollFrame = 0
                const height = window.innerHeight
                const y = window.scrollY
                const atEnd = document.documentElement.scrollHeight - (y + height) <= (footer?.offsetHeight ?? 0) + 1
                const patch: Partial<Motion> = { scrolled: y > 8, footerShown: atEnd }
                if (!navigation.current.locked) {
                    let active = ''
                    if (y > 0 && Math.ceil(y + height) >= document.documentElement.scrollHeight - 2) active = ids.at(-1)!
                    else [prefix + 'top', ...ids].forEach(id => {
                        const section = document.getElementById(id)
                        if (section && section.getBoundingClientRect().top <= height * .4) active = id
                    })
                    patch.active = active
                }
                update(patch)
                updateScene()
            })
        }
        const onResize = () => { measure(); onScroll() }
        window.addEventListener('scroll', onScroll, { passive: true })
        window.addEventListener('resize', onResize, { passive: true })
        reducedMotion.addEventListener('change', onResize)
        measureFrame = requestAnimationFrame(measure)
        void document.fonts.ready.then(measure)
        onScroll()

        const revealAll = () => setMotion(previous => ({ ...previous, seen: Object.fromEntries(ids.map(id => [id, true])) }))
        let observer: IntersectionObserver | undefined
        if (!('IntersectionObserver' in window) || reducedMotion.matches) revealAll()
        else {
            observer = new IntersectionObserver(entries => {
                const visible = entries.filter(entry => entry.isIntersecting)
                if (!visible.length) return
                setMotion(previous => ({
                    ...previous, seen: { ...previous.seen, ...Object.fromEntries(visible.map(entry => [entry.target.id, true])) },
                }))
                visible.forEach(entry => observer?.unobserve(entry.target))
            }, { rootMargin: `0px 0px -${mobile ? 12 : 15}% 0px`, threshold: 0 })
            ids.forEach(id => { const section = document.getElementById(id); if (section) observer?.observe(section) })
        }

        const nav = navigation.current
        return () => {
            disposed = true
            window.removeEventListener('scroll', onScroll)
            window.removeEventListener('resize', onResize)
            reducedMotion.removeEventListener('change', onResize)
            cancelAnimationFrame(scrollFrame)
            cancelAnimationFrame(sceneFrame)
            cancelAnimationFrame(measureFrame)
            window.clearTimeout(nav.timer)
            observer?.disconnect()
        }
    }, [mobile])

    const prefix = mobile ? 'plr-m-' : 'plr-'
    const inView = (id: string) => motion.seen[prefix + id] ? 'is-in' : ''
    const active = (id: string) => motion.active === prefix + id
    const onFaq = (index: number) => () => setFaqOpen(previous => previous === index ? 0 : index)
    const onLegal = (kind: 'terms' | 'privacy') => (event: MouseEvent<HTMLAnchorElement>) => {
        event.preventDefault()
        setLegal(kind)
    }
    const v = {
        accent: 'var(--landing-accent)',
        headerState: motion.scrolled ? 'is-scrolled' : '',
        hintState: motion.scrolled ? 'is-hidden' : '',
        footerState: motion.footerShown ? 'is-shown' : '',
        productsState: inView('products'), verifState: inView('verification'),
        vZoomCls: motion.zoomed ? 'is-zoomed' : '', vSceneCls: `v-s${motion.step}`, vStepTxt: `0${motion.step + 1}`,
        vb: Object.fromEntries([0, 1, 2, 3, 4].map(i => [`b${i}`, i === motion.step ? 'is-active' : i < motion.step ? 'is-done' : ''])),
        vn: Object.fromEntries([0, 1, 2, 3, 4].map(i => [`n${i}`, `${i <= motion.lit ? 'is-lit' : ''}${i === motion.step ? ' is-active' : ''}`])),
        vcur: Object.fromEntries([0, 1, 2, 3, 4].map(i => [`b${i}`, i === motion.step ? 'step' as const : undefined])),
        nav: Object.fromEntries(SECTIONS.map(id => [id === 'verification' ? 'verify' : id, active(id) ? 'is-active' : ''])),
        cur: Object.fromEntries(SECTIONS.map(id => [id === 'verification' ? 'verify' : id, active(id) ? 'location' as const : undefined])),
        sec: Object.fromEntries(SECTIONS.map(id => [id, inView(id)])),
        msec: Object.fromEntries(SECTIONS.map(id => [id, inView(id)])),
        faq: {
            o1: faqOpen === 1, o2: faqOpen === 2, o3: faqOpen === 3, o4: faqOpen === 4, o5: faqOpen === 5,
            c1: faqOpen === 1 ? 'is-open' : '', c2: faqOpen === 2 ? 'is-open' : '', c3: faqOpen === 3 ? 'is-open' : '',
            c4: faqOpen === 4 ? 'is-open' : '', c5: faqOpen === 5 ? 'is-open' : '',
        },
        onFaq1: onFaq(1), onFaq2: onFaq(2), onFaq3: onFaq(3), onFaq4: onFaq(4), onFaq5: onFaq(5),
        onNav: (event: MouseEvent<HTMLAnchorElement>) => {
            const id = event.currentTarget.hash.slice(1)
            navigation.current.locked = true
            setMotion(previous => ({ ...previous, active: id }))
            window.clearTimeout(navigation.current.timer)
            navigation.current.timer = window.setTimeout(() => { navigation.current.locked = false }, 900)
        },
        onTerms: onLegal('terms'), onPrivacy: onLegal('privacy'),
        greenAccent: accent === 'green', toggleAccent,
    }
    return { root, v, legal, closeLegal: () => setLegal(null) }
}

export type LandingValues = ReturnType<typeof useLandingPresentation>['v']
