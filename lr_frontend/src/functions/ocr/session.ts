import * as ort from 'onnxruntime-web/wasm'

/**
 * Shared ONNX Runtime setup for the PrimeLendRow OCR engine.
 *
 * The `onnxruntime-web/wasm` entry is the CPU-only build with the WASM binary
 * inlined, so nothing has to be copied into /public or pointed at by
 * `env.wasm.wasmPaths` — importing it is the whole setup. Threads stay off
 * because SharedArrayBuffer needs COOP/COEP headers the dev server doesn't
 * send; a threaded build would silently fall back anyway.
 *
 * `proxy` moves session creation and inference into a worker. It is not a
 * performance tweak — single-threaded WASM inference is seconds of solid
 * compute, and on the main thread that freezes the whole KYC step: the scan
 * sweep stops, toasts don't paint, nothing responds. Tesseract never showed
 * this because it worker-threaded internally. Keep this on; the CSP already
 * allows the worker via `worker-src 'self' blob:`.
 */
ort.env.wasm.numThreads = 1
ort.env.wasm.proxy = true
ort.env.logLevel = 'error'

const MODEL_BASE = '/models/primelendrow'
export const DET_MODEL_URL = `${MODEL_BASE}/det.onnx`
export const REC_MODEL_URL = `${MODEL_BASE}/rec.onnx`
export const DICT_URL = `${MODEL_BASE}/en_dict.txt`

/**
 * A loaded model plus its single input/output name. The names are read off the
 * session rather than hardcoded ("x", "sigmoid_0.tmp_0", …) so that swapping in
 * a differently-exported checkpoint doesn't need a code change.
 */
export type OcrSession = {
    run: (input: Float32Array, dims: readonly number[]) => Promise<{ data: Float32Array; dims: readonly number[] }>
    dispose: () => Promise<void>
}

export const loadSession = async (url: string): Promise<OcrSession> => {
    const response = await fetch(url)
    if (!response.ok) throw new Error(`Could not fetch ${url} (${response.status}) — run \`npm run models:ocr\` to fetch the weights.`)
    const bytes = new Uint8Array(await response.arrayBuffer())
    const session = await ort.InferenceSession.create(bytes, { executionProviders: [ 'wasm' ], graphOptimizationLevel: 'all' })

    const inputName = session.inputNames[0]
    const outputName = session.outputNames[0]

    return {
        run: async (input, dims) => {
            const output = await session.run({ [inputName]: new ort.Tensor('float32', input, dims as number[]) })
            const tensor = output[outputName]
            return { data: tensor.data as Float32Array, dims: tensor.dims }
        },
        dispose: async () => { await session.release() },
    }
}

/** The recognition charset: CTC blank at index 0, then the dictionary, then the
 * space character PrimeLendRowOCR appends when `use_space_char` is on. The exact tail
 * is confirmed against the model's real class count by `fitCharset`. */
export const loadCharset = async (): Promise<string[]> => {
    const response = await fetch(DICT_URL)
    if (!response.ok) throw new Error(`Could not fetch ${DICT_URL} (${response.status})`)
    const text = await response.text()
    const entries = text.split('\n').map(line => line.replace(/\r$/, '')).filter(line => line.length > 0)
    return [ '<blank>', ...entries, ' ' ]
}

/**
 * PrimeLendRowOCR builds its charset from a dict file plus config flags, so the same
 * dict can legitimately produce N or N+1 classes depending on how the model was
 * exported. Rather than guess, reconcile against the class count the model
 * actually emits — and fail loudly if the gap is bigger than that one optional
 * space, since silently mis-indexing the charset yields plausible-looking
 * gibberish rather than an error.
 */
export const fitCharset = (charset: string[], numClasses: number): string[] => {
    if (charset.length === numClasses) return charset
    if (charset.length === numClasses + 1 && charset.at(-1) === ' ') return charset.slice(0, -1)
    throw new Error(`Recognition charset has ${charset.length} entries but the model emits ${numClasses} classes — wrong dictionary for this checkpoint.`)
}
