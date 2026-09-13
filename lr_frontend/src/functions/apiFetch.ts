import { createEncryptedFetch, type ApiFetch } from './payloadCrypto'

const PAYLOAD_PUBLIC_KEY = import.meta.env.VITE_PAYLOAD_PUBLIC_KEY as string | undefined

/**
 * `fetch` for every call to the engine. With `VITE_PAYLOAD_PUBLIC_KEY` set,
 * request and response bodies are payload-encrypted (see payloadCrypto.ts);
 * without it this is plain `fetch`, so a build without the key keeps working
 * against an engine that doesn't require encryption yet.
 */
export const apiFetch: ApiFetch = PAYLOAD_PUBLIC_KEY
    ? createEncryptedFetch(PAYLOAD_PUBLIC_KEY)
    : (input, init) => fetch(input, init)
