type User = {
    id: string
    username: string
    email: string
    role: string
    expires_at: number
}

const API = import.meta.env.VITE_API_URL ?? ''
const enc = new TextEncoder()

const b64 = (x: ArrayBuffer | Uint8Array) => {
    const bytes = x instanceof Uint8Array ? x : new Uint8Array(x)
    return btoa(String.fromCharCode(...bytes))
}

/** Signs the payload with a fresh Ed25519 key and posts the envelope. */
async function signed(path: string, data: object) {
    const key = await crypto.subtle.generateKey({ name: 'Ed25519' } as AlgorithmIdentifier, true, ['sign', 'verify']) as CryptoKeyPair
    const body = enc.encode(JSON.stringify({ ...data, nonce: crypto.randomUUID(), ingress_expiry: Date.now() + 120000 }))
    const res = await fetch(`${API}${path}`, {
        method: 'POST',
        credentials: 'include',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
            payload: b64(body),
            pubkey: b64(await crypto.subtle.exportKey('raw', key.publicKey)),
            signature: b64(await crypto.subtle.sign('Ed25519', key.privateKey, body)),
        }),
    })
    if (!res.ok) throw new Error(await res.text())
    return res
}

/** Login/verify establish a session, so the response carries a fresh CSRF token alongside the user. */
async function signedSession(path: string, data: object) {
    const res = await signed(path, data)
    return { user: await res.json() as User, csrfToken: res.headers.get('x-csrf-token') }
}

export const loginRequest = (email: string, password: string) => signedSession('/auth/login', { email, password })
export const registerRequest = (name: string, email: string, password: string) => signed('/auth/register', { name, email, password })
export const verifyRequest = (email: string, code: string) => signedSession('/auth/verify', { email, code })
export const resetRequest = (email: string) => signed('/auth/password-reset/request', { email })
export const resetConfirmRequest = (email: string, code: string, password: string) => signed('/auth/password-reset/confirm', { email, code, password })
