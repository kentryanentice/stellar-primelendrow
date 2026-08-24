import { distance, type Quad } from './geometry'
import type { OcrSession } from './session'
import { fitCharset } from './session'

/** The height every crop is scaled to — PP-OCRv3's `rec_image_shape` is [3, 48, W]. */
const REC_HEIGHT = 48
/** Crops are run one at a time at their natural width rather than padded into a
 * fixed 320px batch: a 44-character MRZ line squashed to 320px leaves ~7px per
 * glyph, which is what makes passport numbers come back as noise. */
const MAX_REC_WIDTH = 960
const MIN_REC_WIDTH = 16
/** Taller than it is wide means the line is printed vertically and needs rotating first. */
const VERTICAL_ASPECT = 1.5

export type RecognizedLine = { text: string; confidence: number; quad: Quad }

/**
 * Cuts one detected box out of the full-resolution source and straightens it.
 * The box is a rotated rectangle rather than an arbitrary quadrilateral, so an
 * affine transform reproduces it exactly — no perspective warp needed. The
 * canvas transform maps *image* space to *destination* space, which is the
 * inverse of the corner mapping, hence the explicit inversion below.
 */
const cropAndRectify = (source: HTMLCanvasElement, quad: Quad): HTMLCanvasElement | null => {
    const [ topLeft, topRight, bottomRight, bottomLeft ] = quad
    const width = Math.round(Math.max(distance(topLeft, topRight), distance(bottomLeft, bottomRight)))
    const height = Math.round(Math.max(distance(topLeft, bottomLeft), distance(topRight, bottomRight)))
    if (width < 2 || height < 2) return null

    // destination → source basis vectors
    const ux = (topRight.x - topLeft.x) / width
    const uy = (topRight.y - topLeft.y) / width
    const vx = (bottomLeft.x - topLeft.x) / height
    const vy = (bottomLeft.y - topLeft.y) / height

    const determinant = ux * vy - vx * uy
    if (!determinant) return null

    // invert it, because drawImage transforms source pixels into the destination
    const iux = vy / determinant
    const ivx = -vx / determinant
    const iuy = -uy / determinant
    const ivy = ux / determinant
    const ie = -(iux * topLeft.x + ivx * topLeft.y)
    const iff = -(iuy * topLeft.x + ivy * topLeft.y)

    const canvas = document.createElement('canvas')
    canvas.width = width
    canvas.height = height
    const ctx = canvas.getContext('2d', { willReadFrequently: true })
    if (!ctx) return null
    ctx.imageSmoothingEnabled = true
    ctx.imageSmoothingQuality = 'high'
    ctx.setTransform(iux, iuy, ivx, ivy, ie, iff)
    ctx.drawImage(source, 0, 0)
    ctx.setTransform(1, 0, 0, 1, 0, 0)

    return height / width >= VERTICAL_ASPECT ? rotateQuarterTurn(canvas) : canvas
}

const rotateQuarterTurn = (input: HTMLCanvasElement): HTMLCanvasElement => {
    const canvas = document.createElement('canvas')
    canvas.width = input.height
    canvas.height = input.width
    const ctx = canvas.getContext('2d', { willReadFrequently: true })
    if (!ctx) return input
    ctx.translate(canvas.width / 2, canvas.height / 2)
    ctx.rotate(-Math.PI / 2)
    ctx.drawImage(input, -input.width / 2, -input.height / 2)
    return canvas
}

/** Scales a crop to the recogniser's fixed height and normalises to [-1, 1] in NCHW order. */
const prepareCrop = (crop: HTMLCanvasElement) => {
    const scaled = Math.ceil((REC_HEIGHT * crop.width) / crop.height)
    const width = Math.min(MAX_REC_WIDTH, Math.max(MIN_REC_WIDTH, scaled))

    const canvas = document.createElement('canvas')
    canvas.width = width
    canvas.height = REC_HEIGHT
    const ctx = canvas.getContext('2d', { willReadFrequently: true })
    if (!ctx) throw new Error('Unable to prepare the recognition input')
    ctx.imageSmoothingEnabled = true
    ctx.imageSmoothingQuality = 'high'
    ctx.drawImage(crop, 0, 0, width, REC_HEIGHT)

    const { data } = ctx.getImageData(0, 0, width, REC_HEIGHT)
    const plane = width * REC_HEIGHT
    const input = new Float32Array(3 * plane)
    for (let i = 0, px = 0; i < plane; i++, px += 4) {
        input[i] = (data[px] / 255 - 0.5) / 0.5
        input[plane + i] = (data[px + 1] / 255 - 0.5) / 0.5
        input[2 * plane + i] = (data[px + 2] / 255 - 0.5) / 0.5
    }
    return { input, width }
}

/**
 * Greedy CTC decoding: take the best class at each timestep, collapse runs of
 * the same class, and drop the blank. Confidence is the mean probability of the
 * timesteps that actually produced a character — averaging over the blanks
 * instead would report ~99% for every line, since most timesteps are blank.
 */
const decodeCtc = (logits: Float32Array, timesteps: number, numClasses: number, charset: string[]) => {
    let text = ''
    let probabilitySum = 0
    let kept = 0
    let previous = -1

    for (let t = 0; t < timesteps; t++) {
        const offset = t * numClasses
        let bestIndex = 0
        let bestProbability = logits[offset]
        for (let c = 1; c < numClasses; c++) {
            if (logits[offset + c] > bestProbability) {
                bestProbability = logits[offset + c]
                bestIndex = c
            }
        }
        if (bestIndex !== 0 && bestIndex !== previous) {
            text += charset[bestIndex] ?? ''
            probabilitySum += bestProbability
            kept++
        }
        previous = bestIndex
    }
    return { text: text.trim(), confidence: kept ? probabilitySum / kept : 0 }
}

export const recognize = async (
    session: OcrSession,
    source: HTMLCanvasElement,
    quads: Quad[],
    charset: string[],
): Promise<RecognizedLine[]> => {
    const lines: RecognizedLine[] = []
    let resolvedCharset = charset

    for (const quad of quads) {
        const crop = cropAndRectify(source, quad)
        if (!crop) continue
        const { input, width } = prepareCrop(crop)
        const { data, dims } = await session.run(input, [ 1, 3, REC_HEIGHT, width ])

        const timesteps = dims[1]
        const numClasses = dims[2]
        resolvedCharset = fitCharset(resolvedCharset, numClasses)

        const { text, confidence } = decodeCtc(data, timesteps, numClasses, resolvedCharset)
        if (text) lines.push({ text, confidence, quad })
    }
    return lines
}
