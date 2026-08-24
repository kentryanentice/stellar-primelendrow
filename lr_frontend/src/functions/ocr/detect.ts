import { minAreaRect, unclipRect, type Point, type Quad } from './geometry'
import type { OcrSession } from './session'

/** DB predicts a shrunken probability map; this is the cut that turns it into a mask. */
const BINARIZE_THRESHOLD = 0.3
/** Mean probability a region must reach to be kept — filters halos around real text. */
const BOX_SCORE_THRESHOLD = 0.5
/** How far to dilate each shrunken region back out (PrimelendRowOCR's `det_db_unclip_ratio`). */
const UNCLIP_RATIO = 1.5
/** Boxes thinner than this are detector noise, never a readable line of text. */
const MIN_BOX_SIDE = 3
/** Regions smaller than this can't hold a character at detection scale. */
const MIN_COMPONENT_PIXELS = 12
/** PP-OCR's `det_limit_side_len`: the longest side is capped here before inference. */
const MAX_SIDE = 960
/** The detector's downsampling stack requires both input dimensions to be multiples of 32. */
const STRIDE = 32

const MEAN = [ 0.485, 0.456, 0.406 ]
const STD = [ 0.229, 0.224, 0.225 ]

export type DetectedBox = { quad: Quad; score: number }

/**
 * Resizes into the detector's expected input and converts to normalised NCHW
 * float. Returns the scale factors too, since every box comes back in this
 * resized frame and has to be mapped onto the full-resolution source before
 * cropping.
 */
const prepareInput = (source: HTMLCanvasElement) => {
    const longest = Math.max(source.width, source.height)
    const ratio = longest > MAX_SIDE ? MAX_SIDE / longest : 1
    const width = Math.max(STRIDE, Math.round((source.width * ratio) / STRIDE) * STRIDE)
    const height = Math.max(STRIDE, Math.round((source.height * ratio) / STRIDE) * STRIDE)

    const canvas = document.createElement('canvas')
    canvas.width = width
    canvas.height = height
    const ctx = canvas.getContext('2d', { willReadFrequently: true })
    if (!ctx) throw new Error('Unable to prepare the detection input')
    ctx.imageSmoothingEnabled = true
    ctx.imageSmoothingQuality = 'high'
    ctx.drawImage(source, 0, 0, width, height)

    const { data } = ctx.getImageData(0, 0, width, height)
    const plane = width * height
    const input = new Float32Array(3 * plane)
    for (let i = 0, px = 0; i < plane; i++, px += 4) {
        input[i] = (data[px] / 255 - MEAN[0]) / STD[0]
        input[plane + i] = (data[px + 1] / 255 - MEAN[1]) / STD[1]
        input[2 * plane + i] = (data[px + 2] / 255 - MEAN[2]) / STD[2]
    }
    return { input, width, height }
}

/**
 * Groups above-threshold pixels into connected regions and fits a rotated
 * rectangle to each. This replaces OpenCV's `findContours` + `minAreaRect`:
 * flood-filling gives the same regions, and the min-area rect of a region's
 * pixels equals that of its contour, so nothing is lost by skipping the
 * intermediate outline.
 */
const boxesFromProbabilityMap = (prob: Float32Array, width: number, height: number): DetectedBox[] => {
    const visited = new Uint8Array(width * height)
    const boxes: DetectedBox[] = []
    const stack: number[] = []

    for (let start = 0; start < prob.length; start++) {
        if (visited[start] || prob[start] <= BINARIZE_THRESHOLD) continue

        visited[start] = 1
        stack.length = 0
        stack.push(start)
        const points: Point[] = []
        let scoreSum = 0

        while (stack.length) {
            const index = stack.pop()!
            const x = index % width
            const y = (index - x) / width
            points.push({ x, y })
            scoreSum += prob[index]

            // 8-connectivity, matching how findContours would trace the region
            for (let dy = -1; dy <= 1; dy++) {
                for (let dx = -1; dx <= 1; dx++) {
                    if (dx === 0 && dy === 0) continue
                    const nx = x + dx
                    const ny = y + dy
                    if (nx < 0 || ny < 0 || nx >= width || ny >= height) continue
                    const neighbour = ny * width + nx
                    if (visited[neighbour] || prob[neighbour] <= BINARIZE_THRESHOLD) continue
                    visited[neighbour] = 1
                    stack.push(neighbour)
                }
            }
        }

        if (points.length < MIN_COMPONENT_PIXELS) continue
        const score = scoreSum / points.length
        if (score < BOX_SCORE_THRESHOLD) continue

        const rect = minAreaRect(points)
        if (!rect || Math.min(rect.width, rect.height) < MIN_BOX_SIDE) continue

        const expanded = unclipRect(rect.quad, rect.width, rect.height, UNCLIP_RATIO)
        if (Math.min(expanded.width, expanded.height) < MIN_BOX_SIDE) continue
        boxes.push({ quad: expanded.quad, score })
    }
    return boxes
}

/** Maps boxes from the resized detection frame back onto the source image, clamped to its bounds. */
const rescale = (boxes: DetectedBox[], scaleX: number, scaleY: number, maxX: number, maxY: number): DetectedBox[] =>
    boxes.map(({ quad, score }) => ({
        score,
        quad: quad.map(p => ({
            x: Math.min(maxX, Math.max(0, p.x * scaleX)),
            y: Math.min(maxY, Math.max(0, p.y * scaleY)),
        })) as unknown as Quad,
    }))

export const detect = async (session: OcrSession, source: HTMLCanvasElement): Promise<DetectedBox[]> => {
    const { input, width, height } = prepareInput(source)
    const { data, dims } = await session.run(input, [ 1, 3, height, width ])

    // output is [1, 1, H, W] — the same spatial size as the input
    const outHeight = dims[dims.length - 2]
    const outWidth = dims[dims.length - 1]
    const boxes = boxesFromProbabilityMap(data, outWidth, outHeight)

    return rescale(boxes, source.width / outWidth, source.height / outHeight, source.width, source.height)
        .filter(box => box.quad.every(p => Number.isFinite(p.x) && Number.isFinite(p.y)))
}
