import type { Page as OcrPage } from 'tesseract.js'
import { preprocessForOcr } from '../KYC/image'
import { detect } from './detect'
import { recognize } from './recognize'
import { toOcrPage } from './toOcrPage'
import { DET_MODEL_URL, REC_MODEL_URL, loadCharset, loadSession, type OcrSession } from './session'

/**
 * The one seam every OCR engine fits through. `parseIdText` consumes
 * Tesseract's `Page` shape, so this engine adapts its detection boxes into the
 * same lines/words/confidence tree rather than getting its own parser — which
 * is also what lets the dev harness score both engines through identical
 * downstream parsing.
 */
export type OcrEngine = {
    name: string
    /** Anything `new Image().src = ...` accepts: a data URL or a served URL. */
    recognize: (src: string) => Promise<OcrPage>
    dispose: () => Promise<void>
}

/**
 * The PrimeLendRow OCR engine: a two-stage detect-then-recognise pipeline
 * running on onnxruntime-web.
 *
 * Under the hood the weights are PP-OCRv4 detection and PP-OCRv3 English
 * recognition (Apache-2.0, see scripts/fetch-ocr-models.mjs for the exact
 * upstream files) — recorded here because a maintainer debugging a bad box or
 * swapping a checkpoint needs to know which upstream this tracks. Detection is
 * language-agnostic, so the stronger v4 detector pairs with the v3 English
 * recogniser: its 95-character Latin dictionary is a far better fit for
 * Philippine IDs than the 6000-plus character Chinese default, and a smaller
 * charset is also a smaller space for CTC to hallucinate in.
 *
 * `preprocessForOcr` runs first, exactly as it did for Tesseract. That greys
 * the image, which throws away colour the detector could otherwise use —
 * passing the raw source here instead is the first knob worth turning if
 * detection comes back sparse, but it should be measured in the harness before
 * being changed, not swapped on intuition.
 */
export const createOcrEngine = async (): Promise<OcrEngine> => {
    let detSession: OcrSession | null = null
    let recSession: OcrSession | null = null
    let charset: string[] | null = null

    const ready = async () => {
        if (!detSession) detSession = await loadSession(DET_MODEL_URL)
        if (!recSession) recSession = await loadSession(REC_MODEL_URL)
        if (!charset) charset = await loadCharset()
        return { detSession, recSession, charset }
    }

    return {
        name: 'primelendrow (det + en rec)',
        recognize: async src => {
            const { detSession: det, recSession: rec, charset: chars } = await ready()
            const source = await preprocessForOcr(src)
            const boxes = await detect(det, source)
            const lines = await recognize(rec, source, boxes.map(box => box.quad), chars)
            return toOcrPage(lines)
        },
        dispose: async () => {
            await detSession?.dispose()
            await recSession?.dispose()
            detSession = null
            recSession = null
        },
    }
}

/**
 * Process-wide engine, because the weights are ~14MB: Tesseract span a fresh
 * worker per scan and threw it away, which is affordable for a 4MB traineddata
 * the browser caches but not for two ONNX sessions that have to be parsed and
 * graph-optimised each time. Holding one engine means only the first scan of a
 * session pays that cost, and a retry after a failed scan is near-instant.
 *
 * The promise itself is cached rather than the resolved engine, so two scans
 * fired before the first load settles share one load instead of racing into two.
 */
let engine: Promise<OcrEngine> | null = null

const sharedEngine = () => {
    // a failed load must not be cached, or every later scan replays the same error
    engine ??= createOcrEngine().catch(err => { engine = null; throw err })
    return engine
}

/** Reads an ID image with the shared engine. This is what production KYC calls. */
export const recognizeIdImage = async (src: string): Promise<OcrPage> => (await sharedEngine()).recognize(src)

/**
 * Starts loading the models without doing a scan, so the download overlaps the
 * time the user spends framing and confirming their ID instead of landing
 * entirely on the first tap of Scan. Safe to call repeatedly; failures are
 * swallowed because this is only ever an optimisation — `recognizeIdImage`
 * reports the real error.
 */
export const warmOcrEngine = () => { void sharedEngine().catch(() => {}) }
