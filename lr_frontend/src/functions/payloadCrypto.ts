/**
 * Application-layer payload encryption for calls to the engine — the browser
 * half of lr_engine/src/infra/payload.rs, which documents the scheme in full.
 *
 * Every call makes a one-off P-256 key pair, runs ECDH against the engine's
 * public key (pinned at build time, never fetched), and derives one
 * AES-256-GCM key per direction with HKDF-SHA256. The request body goes out as
 * raw `iv || ciphertext`; the response comes back the same way and is handed
 * to the caller as an ordinary `Response`, so `res.ok`, `res.json()`,
 * `res.text()` and `res.headers.get(...)` behave exactly as they did with
 * plain `fetch`.
 *
 * Kept free of `import.meta.env` so it runs outside Vite too.
 */

const encoder = new TextEncoder()
const IV_LENGTH = 12
const HKDF_SALT = new Uint8Array(32)
const REQUEST_INFO = encoder.encode('primelendrow payload v1 request')
const RESPONSE_INFO = encoder.encode('primelendrow payload v1 response')

const fromBase64 = (value: string) => Uint8Array.from(atob(value.trim()), c => c.charCodeAt(0))

const toBase64 = (bytes: Uint8Array) => {
    let binary = ''
    for (const byte of bytes) binary += String.fromCharCode(byte)
    return btoa(binary)
}

export type ApiFetch = (input: string, init?: RequestInit) => Promise<Response>

/**
 * The engine's tunnel address (`TUNNEL_PATH` in payload.rs). With tunnelling
 * on, every call leaves the browser as `POST <origin>/x`: the real method,
 * path and query string travel inside the sealed body, ahead of the real
 * body, so the network tab shows one meaningless address instead of the API's
 * route names. Framing: `u32 big-endian head length || head JSON || body`.
 */
const TUNNEL_PATH = '/x'

export type EncryptedFetchOptions = { tunnel?: boolean }

export function createEncryptedFetch(
    serverPublicKey: string,
    baseFetch: typeof fetch = fetch,
    { tunnel = false }: EncryptedFetchOptions = {},
): ApiFetch {
    // Imported once; a malformed key rejects on the first call rather than at load.
    const serverKey = crypto.subtle.importKey('raw', fromBase64(serverPublicKey), { name: 'ECDH', namedCurve: 'P-256' }, false, [])

    return async (input, init = {}) => {
        const method = (init.method ?? 'GET').toUpperCase()
        const url = new URL(input, globalThis.location?.href)
        // What the browser actually sends: the tunnel address, or the call itself.
        const wireMethod = tunnel ? 'POST' : method
        const wirePath = tunnel ? TUNNEL_PATH : url.pathname
        const requestAad = `${wireMethod} ${wirePath}`

        const ephemeral = await crypto.subtle.generateKey({ name: 'ECDH', namedCurve: 'P-256' }, false, ['deriveBits']) as CryptoKeyPair
        const shared = await crypto.subtle.deriveBits({ name: 'ECDH', public: await serverKey }, ephemeral.privateKey, 256)
        const hkdf = await crypto.subtle.importKey('raw', shared, 'HKDF', false, ['deriveKey'])
        const derive = (info: Uint8Array<ArrayBuffer>, usage: KeyUsage) =>
            crypto.subtle.deriveKey({ name: 'HKDF', hash: 'SHA-256', salt: HKDF_SALT, info }, hkdf, { name: 'AES-GCM', length: 256 }, false, [usage])
        const [requestKey, responseKey] = await Promise.all([derive(REQUEST_INFO, 'encrypt'), derive(RESPONSE_INFO, 'decrypt')])

        const headers = new Headers(init.headers)
        headers.set('x-payload-key', toBase64(new Uint8Array(await crypto.subtle.exportKey('raw', ephemeral.publicKey))))

        const seal = async (plain: Uint8Array<ArrayBuffer>) => {
            const iv = crypto.getRandomValues(new Uint8Array(IV_LENGTH))
            const ciphertext = new Uint8Array(await crypto.subtle.encrypt(
                { name: 'AES-GCM', iv, additionalData: encoder.encode(requestAad) },
                requestKey,
                plain,
            ))
            const sealed = new Uint8Array(IV_LENGTH + ciphertext.length)
            sealed.set(iv)
            sealed.set(ciphertext, IV_LENGTH)
            return sealed
        }

        let body = init.body
        // Every engine call sends JSON.stringify(...) — anything else is a
        // new caller that needs thought, not silent plaintext.
        if (body != null && typeof body !== 'string') throw new TypeError('apiFetch only encrypts string bodies')

        if (tunnel) {
            // Always sealed, even a GET: the head is what's being hidden.
            const bodyBytes = body != null ? encoder.encode(body) : new Uint8Array(0)
            const head = encoder.encode(JSON.stringify({
                m: method,
                p: `${url.pathname}${url.search}`,
                ...(body != null ? { t: headers.get('content-type') ?? 'application/json' } : {}),
            }))
            const frame = new Uint8Array(4 + head.length + bodyBytes.length)
            new DataView(frame.buffer).setUint32(0, head.length)
            frame.set(head, 4)
            frame.set(bodyBytes, 4 + head.length)
            headers.delete('x-payload-type')
            headers.set('content-type', 'application/octet-stream')
            headers.set('x-payload-enc', '1')
            body = await seal(frame)
        } else if (body != null) {
            headers.set('x-payload-type', headers.get('content-type') ?? 'application/json')
            headers.set('content-type', 'application/octet-stream')
            headers.set('x-payload-enc', '1')
            body = await seal(encoder.encode(body))
        }

        const target = tunnel ? `${url.origin}${TUNNEL_PATH}` : input
        const res = await baseFetch(target, { ...init, method: wireMethod, headers, body })

        // Rejections from outside the payload layer (CORS, CSRF, rate limits)
        // and empty bodies arrive unsealed — hand those back untouched.
        if (res.headers.get('x-payload-enc') !== '1') return res

        const sealed = new Uint8Array(await res.arrayBuffer())
        if (sealed.length <= IV_LENGTH) throw new Error('Malformed encrypted response')
        const plain = await crypto.subtle.decrypt(
            { name: 'AES-GCM', iv: sealed.subarray(0, IV_LENGTH), additionalData: encoder.encode(`${requestAad} ${res.status}`) },
            responseKey,
            sealed.subarray(IV_LENGTH),
        )

        const responseHeaders = new Headers(res.headers)
        responseHeaders.set('content-type', res.headers.get('x-payload-type') ?? 'application/json')
        responseHeaders.delete('x-payload-enc')
        responseHeaders.delete('x-payload-type')
        return new Response(plain, { status: res.status, statusText: res.statusText, headers: responseHeaders })
    }
}
