export type Point = { x: number; y: number }
/** A rotated rectangle's corners, ordered top-left, top-right, bottom-right, bottom-left. */
export type Quad = [Point, Point, Point, Point]

export const distance = (a: Point, b: Point) => Math.hypot(a.x - b.x, a.y - b.y)

const cross = (o: Point, a: Point, b: Point) => (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)

/** Andrew's monotone chain. The hull of a blob's pixels is the same as the hull
 * of its outline, so callers can pass every pixel and skip contour tracing. */
export const convexHull = (points: Point[]): Point[] => {
    if (points.length < 3) return points.slice()
    const sorted = points.slice().sort((a, b) => (a.x - b.x) || (a.y - b.y))

    const half = (input: Point[]) => {
        const out: Point[] = []
        for (const p of input) {
            while (out.length >= 2 && cross(out[out.length - 2], out[out.length - 1], p) <= 0) out.pop()
            out.push(p)
        }
        out.pop()
        return out
    }
    return [ ...half(sorted), ...half(sorted.reverse()) ]
}

/**
 * Minimum-area enclosing rectangle by rotating calipers: the optimal rectangle
 * always shares an edge with the convex hull, so testing the axis-aligned
 * bounding box in each edge's frame and keeping the smallest is exact, not an
 * approximation. This stands in for OpenCV's `minAreaRect`, which the DB
 * post-processing step would otherwise need.
 */
export const minAreaRect = (points: Point[]): { quad: Quad; width: number; height: number } | null => {
    const hull = convexHull(points)
    if (hull.length < 3) return null

    let best: { angle: number; minU: number; maxU: number; minV: number; maxV: number; area: number } | null = null

    for (let i = 0; i < hull.length; i++) {
        const a = hull[i]
        const b = hull[(i + 1) % hull.length]
        const angle = Math.atan2(b.y - a.y, b.x - a.x)
        const cos = Math.cos(-angle)
        const sin = Math.sin(-angle)

        let minU = Infinity, maxU = -Infinity, minV = Infinity, maxV = -Infinity
        for (const p of hull) {
            const u = p.x * cos - p.y * sin
            const v = p.x * sin + p.y * cos
            if (u < minU) minU = u
            if (u > maxU) maxU = u
            if (v < minV) minV = v
            if (v > maxV) maxV = v
        }
        const area = (maxU - minU) * (maxV - minV)
        if (!best || area < best.area) best = { angle, minU, maxU, minV, maxV, area }
    }
    if (!best) return null

    // rotate the corners of the best frame's bounding box back into image space
    const cos = Math.cos(best.angle)
    const sin = Math.sin(best.angle)
    const toImage = (u: number, v: number): Point => ({ x: u * cos - v * sin, y: u * sin + v * cos })
    const corners = [
        toImage(best.minU, best.minV),
        toImage(best.maxU, best.minV),
        toImage(best.maxU, best.maxV),
        toImage(best.minU, best.maxV),
    ]
    return { quad: orderQuad(corners), width: best.maxU - best.minU, height: best.maxV - best.minV }
}

/**
 * Puts four corners into top-left, top-right, bottom-right, bottom-left order.
 * Splitting on x first and only then on y (rather than sorting by angle around
 * the centroid) keeps the labelling stable for the near-horizontal boxes text
 * detection produces, where all four corners can share almost the same y.
 */
export const orderQuad = (corners: Point[]): Quad => {
    const byX = corners.slice().sort((a, b) => a.x - b.x)
    const [ left, right ] = [ byX.slice(0, 2), byX.slice(2) ]
    const [ topLeft, bottomLeft ] = left.sort((a, b) => a.y - b.y)
    const [ topRight, bottomRight ] = right.sort((a, b) => a.y - b.y)
    return [ topLeft, topRight, bottomRight, bottomLeft ]
}

/**
 * DB's "unclip" step: the network is trained to predict a shrunken version of
 * each text region, so every box has to be dilated back out before cropping or
 * the first and last characters get sliced off. PrimeLendRowOCR runs a pyclipper
 * polygon offset by `area * ratio / perimeter`; for a rectangle that offset is
 * exactly an outset of `distance` on all four sides, so this is the same
 * result without the polygon-clipping dependency.
 */
export const unclipRect = (quad: Quad, width: number, height: number, ratio: number): { quad: Quad; width: number; height: number } => {
    const area = width * height
    const perimeter = 2 * (width + height)
    if (perimeter === 0) return { quad, width, height }
    const distanceOut = (area * ratio) / perimeter

    const center: Point = {
        x: (quad[0].x + quad[1].x + quad[2].x + quad[3].x) / 4,
        y: (quad[0].y + quad[1].y + quad[2].y + quad[3].y) / 4,
    }
    // unit vectors along the box's own axes, so the outset follows its rotation
    const alongX = { x: (quad[1].x - quad[0].x) / (width || 1), y: (quad[1].y - quad[0].y) / (width || 1) }
    const alongY = { x: (quad[3].x - quad[0].x) / (height || 1), y: (quad[3].y - quad[0].y) / (height || 1) }

    const halfWidth = width / 2 + distanceOut
    const halfHeight = height / 2 + distanceOut
    const corner = (sx: number, sy: number): Point => ({
        x: center.x + alongX.x * sx * halfWidth + alongY.x * sy * halfHeight,
        y: center.y + alongX.y * sx * halfWidth + alongY.y * sy * halfHeight,
    })

    return {
        quad: [ corner(-1, -1), corner(1, -1), corner(1, 1), corner(-1, 1) ],
        width: width + 2 * distanceOut,
        height: height + 2 * distanceOut,
    }
}
