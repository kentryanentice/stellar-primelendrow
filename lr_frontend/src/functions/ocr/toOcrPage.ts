import type { Page as OcrPage } from 'tesseract.js'
import type { Quad } from './geometry'
import type { RecognizedLine } from './recognize'

/**
 * Adapts PrimeLendRow OCR output into the Tesseract `Page` shape that
 * `parseIdText` already consumes, so both engines are scored through
 * byte-identical parsing and any score difference is attributable to
 * recognition alone.
 *
 * The two engines disagree about what a "line" is, and reconciling that is the
 * whole job here. Tesseract returns one line per row of the page, so two fields
 * printed side by side ("Given Names" next to "Middle Name") arrive fused into
 * a single line — which is exactly what `parseIdText`'s combined-label branch
 * and `splitIntoColumns` are written against. This engine instead detects every
 * text region separately, so those same two labels arrive as two boxes. Merging
 * boxes that share a row back into one line is therefore not a convenience: it
 * is what keeps the existing parser's column logic reachable at all.
 */

const bounds = (quad: Quad) => {
    const xs = quad.map(p => p.x)
    const ys = quad.map(p => p.y)
    return { x0: Math.min(...xs), x1: Math.max(...xs), y0: Math.min(...ys), y1: Math.max(...ys) }
}

type Placed = RecognizedLine & { x0: number; x1: number; y0: number; y1: number }

/**
 * Groups boxes into rows by vertical overlap rather than by a fixed y
 * tolerance: ID cards mix large header type with small field print, so any
 * single pixel threshold either splits a row of big text or fuses two rows of
 * small text.
 */
const intoRows = (placed: Placed[]): Placed[][] => {
    const rows: Placed[][] = []
    for (const item of placed.slice().sort((a, b) => a.y0 - b.y0)) {
        const row = rows.find(candidate => {
            const top = Math.min(...candidate.map(c => c.y0))
            const bottom = Math.max(...candidate.map(c => c.y1))
            const overlap = Math.min(bottom, item.y1) - Math.max(top, item.y0)
            return overlap > 0.5 * Math.min(bottom - top, item.y1 - item.y0)
        })
        if (row) row.push(item)
        else rows.push([ item ])
    }
    return rows.map(row => row.sort((a, b) => a.x0 - b.x0))
}

/**
 * Splits a recognised box into words, estimating each word's left edge by
 * character offset across the box. The detector reports geometry per text region,
 * not per word, but `splitIntoColumns` needs per-word x positions to find the
 * gap between two columns — and proportional spacing recovers that well enough,
 * because the gap it looks for is far wider than the error this introduces.
 */
const wordsOf = (line: Placed) => {
    const width = line.x1 - line.x0
    const total = line.text.length || 1
    const words: { text: string; confidence: number; bbox: { x0: number; y0: number; x1: number; y1: number } }[] = []

    let cursor = 0
    for (const token of line.text.split(/\s+/)) {
        if (!token) continue
        const start = line.text.indexOf(token, cursor)
        const at = start === -1 ? cursor : start
        cursor = at + token.length
        words.push({
            text: token,
            // the recogniser scores 0–1; Tesseract's scale (and MIN_WORD_CONFIDENCE) is 0–100
            confidence: line.confidence * 100,
            bbox: {
                x0: line.x0 + (at / total) * width,
                x1: line.x0 + (cursor / total) * width,
                y0: line.y0,
                y1: line.y1,
            },
        })
    }
    return words
}

export const toOcrPage = (recognized: RecognizedLine[]): OcrPage => {
    const placed: Placed[] = recognized.map(line => ({ ...line, ...bounds(line.quad) }))
    const rows = intoRows(placed)

    const lines = rows.map(row => {
        const words = row.flatMap(wordsOf)
        const text = row.map(item => item.text).join(' ')
        const confidence = row.reduce((sum, item) => sum + item.confidence, 0) / (row.length || 1) * 100
        return {
            text,
            confidence,
            words,
            bbox: {
                x0: Math.min(...row.map(i => i.x0)),
                x1: Math.max(...row.map(i => i.x1)),
                y0: Math.min(...row.map(i => i.y0)),
                y1: Math.max(...row.map(i => i.y1)),
            },
        }
    })

    const text = lines.map(line => line.text).join('\n')
    const confidence = lines.length ? lines.reduce((sum, l) => sum + l.confidence, 0) / lines.length : 0

    // Only the fields `parseIdText` actually reads are populated; the rest of
    // Tesseract's Page surface (hocr, tsv, pdf, …) has no equivalent here,
    // hence the cast rather than a pile of nulls that would still need one.
    return {
        text,
        confidence,
        blocks: [ { paragraphs: [ { lines, text, confidence } ], text, confidence } ],
    } as unknown as OcrPage
}
