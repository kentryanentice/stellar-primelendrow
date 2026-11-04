import { useEffect, type RefObject } from 'react'

type Position = { rx: number; ry: number; lift: number; mx: number; my: number }
type Surface = {
    element: HTMLElement
    bounds: DOMRect | null
    hovered: boolean
    current: Position
    target: Position
}

const INITIAL: Position = { rx: 0, ry: 0, lift: 0, mx: 50, my: 0 }
const KEYS = ['rx', 'ry', 'lift', 'mx', 'my'] as const

// Keep pointer motion off React's render path. Both card types use the same
// frame-rate-independent damping, including the return to rest on pointer leave.
export function useSurfaceTilt(ref: RefObject<HTMLElement | null>, lift: number, selector?: string) {
    useEffect(() => {
        const root = ref.current
        if (!root) return

        const media = window.matchMedia('(hover: hover) and (pointer: fine) and (min-width: 721px) and (prefers-reduced-motion: no-preference)')
        const elements = selector ? Array.from(root.querySelectorAll<HTMLElement>(selector)) : [root]
        const active = new Set<Surface>()
        let frame = 0
        let lastTime = 0

        const animate = (time: number) => {
            const elapsed = lastTime ? Math.min(64, time - lastTime) : 16
            lastTime = time
            const damping = 1 - Math.exp(-elapsed / 110)
            let moving = false

            active.forEach((surface) => {
                let settling = false
                KEYS.forEach((key) => {
                    const distance = surface.target[key] - surface.current[key]
                    if (Math.abs(distance) > .01) {
                        surface.current[key] += distance * damping
                        settling = true
                    } else {
                        surface.current[key] = surface.target[key]
                    }
                })

                const { current, element } = surface
                element.style.setProperty('--rx', `${current.rx.toFixed(3)}deg`)
                element.style.setProperty('--ry', `${current.ry.toFixed(3)}deg`)
                element.style.setProperty('--lift', `${current.lift.toFixed(2)}px`)
                element.style.setProperty('--mx', `${current.mx.toFixed(1)}%`)
                element.style.setProperty('--my', `${current.my.toFixed(1)}%`)

                moving ||= settling
                if (!settling && !surface.hovered) active.delete(surface)
            })

            frame = moving ? requestAnimationFrame(animate) : 0
            if (!frame) lastTime = 0
        }

        const start = () => {
            if (!frame) frame = requestAnimationFrame(animate)
        }

        const surfaces = elements.map((element) => {
            const surface: Surface = {
                element, bounds: null, hovered: false,
                current: { ...INITIAL }, target: { ...INITIAL },
            }

            const track = (event: PointerEvent) => {
                if (event.pointerType !== 'mouse' || !media.matches) return
                if (!surface.hovered) surface.bounds = null
                const bounds = surface.bounds ??= element.getBoundingClientRect()
                if (!bounds.width || !bounds.height) return

                const x = Math.min(.5, Math.max(-.5, (event.clientX - bounds.left) / bounds.width - .5))
                const y = Math.min(.5, Math.max(-.5, (event.clientY - bounds.top) / bounds.height - .5))
                surface.target = { rx: -y * 6, ry: x * 8, lift, mx: (x + .5) * 100, my: (y + .5) * 100 }
                surface.hovered = true
                element.dataset.tiltActive = 'true'
                active.add(surface)
                start()
            }

            const leave = () => {
                surface.hovered = false
                surface.bounds = null
                delete element.dataset.tiltActive
                surface.target.rx = surface.target.ry = surface.target.lift = 0
                if (active.has(surface)) start()
            }

            element.addEventListener('pointerenter', track)
            element.addEventListener('pointermove', track)
            element.addEventListener('pointerleave', leave)
            element.addEventListener('pointercancel', leave)
            return { surface, track, leave }
        })

        const reset = () => {
            cancelAnimationFrame(frame)
            frame = lastTime = 0
            active.clear()
            surfaces.forEach(({ surface }) => {
                surface.hovered = false
                surface.bounds = null
                surface.current = { ...INITIAL }
                surface.target = { ...INITIAL }
                delete surface.element.dataset.tiltActive
                KEYS.forEach((key) => surface.element.style.removeProperty(`--${key}`))
            })
        }

        const invalidateBounds = () => surfaces.forEach(({ surface }) => { surface.bounds = null })
        media.addEventListener('change', reset)
        window.addEventListener('scroll', invalidateBounds, { passive: true })
        window.addEventListener('resize', invalidateBounds, { passive: true })
        window.addEventListener('blur', reset)

        return () => {
            reset()
            media.removeEventListener('change', reset)
            window.removeEventListener('scroll', invalidateBounds)
            window.removeEventListener('resize', invalidateBounds)
            window.removeEventListener('blur', reset)
            surfaces.forEach(({ surface: { element }, track, leave }) => {
                element.removeEventListener('pointerenter', track)
                element.removeEventListener('pointermove', track)
                element.removeEventListener('pointerleave', leave)
                element.removeEventListener('pointercancel', leave)
            })
        }
    }, [ref, lift, selector])
}
