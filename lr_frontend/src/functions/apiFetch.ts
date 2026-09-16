import { createEncryptedFetch, type ApiFetch } from './payloadCrypto'

const PAYLOAD_PUBLIC_KEY = import.meta.env.VITE_PAYLOAD_PUBLIC_KEY as string | undefined
/** On unless set to "false" — the escape hatch for an engine without `/x`. */
const PAYLOAD_TUNNEL = import.meta.env.VITE_PAYLOAD_TUNNEL !== 'false'

/**
 * `fetch` for every call to the engine. With `VITE_PAYLOAD_PUBLIC_KEY` set,
 * request and response bodies are payload-encrypted (see payloadCrypto.ts)
 * and, unless `VITE_PAYLOAD_TUNNEL=false`, every call goes through the
 * engine's tunnel so the network tab shows `POST /x` rather than the route.
 * Without the key this is plain `fetch`, so a build without the key keeps
 * working against an engine that doesn't require encryption yet.
 */
export const apiFetch: ApiFetch = PAYLOAD_PUBLIC_KEY
    ? createEncryptedFetch(PAYLOAD_PUBLIC_KEY, undefined, { tunnel: PAYLOAD_TUNNEL })
    : (input, init) => fetch(input, init)
