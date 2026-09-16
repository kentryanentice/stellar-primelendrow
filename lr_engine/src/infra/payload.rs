//! Application-layer payload encryption between the browser and the engine.
//!
//! TLS already encrypts the wire; this is a second layer that holds even
//! where TLS has been opened up — a TLS-terminating proxy or CDN, a logging
//! middlebox, an intercepting certificate — because only this server holds
//! the private key payloads are sealed to. The frontend pins the matching
//! public key at build time (`VITE_PAYLOAD_PUBLIC_KEY`), so a key swapped in
//! transit is never trusted.
//!
//! Per request (lr_frontend/src/functions/payloadCrypto.ts is the other half):
//!   1. The browser makes a one-off P-256 key pair and sends its public half
//!      in `x-payload-key` (base64, uncompressed SEC1 point).
//!   2. Both sides run ECDH against the server's static key, then HKDF-SHA256
//!      (32 zero bytes of salt) with two labels — one AES-256-GCM key per
//!      direction, so a response can never be replayed back in as a request.
//!   3. A request body travels as raw `iv(12) || ciphertext`, flagged by
//!      `x-payload-enc: 1`, with its real content type moved to
//!      `x-payload-type`. Raw bytes rather than base64 keep the 24MB KYC
//!      upload inside its limit.
//!   4. The response body is sealed the same way under the response key.
//!
//! The GCM associated data is `"<METHOD> <path>"` on requests and
//! `"<METHOD> <path> <status>"` on responses: a ciphertext lifted from one
//! endpoint fails to open on another, and a response's status can't be
//! rewritten (a refused withdrawal passed off as a success) undetected.
//! Only bodies are covered — query strings, path parameters, cookies and
//! headers are not.
//!
//! What stays in the clear, deliberately:
//! - rejections produced outside this layer (CORS, CSRF, rate and concurrency
//!   limits), which carry a status and a fixed message and nothing else;
//! - `EXEMPT_PATHS`: Stripe's webhook and the PayPal/Stripe redirect landings,
//!   reached by a provider or a top-level browser navigation that could never
//!   send a payload key.
//!
//! Rollout: with `PAYLOAD_PRIVATE_KEY` set, encrypted and plaintext requests
//! are both accepted, so the engine can ship before the frontend does. Once
//! every client encrypts, `PAYLOAD_ENCRYPTION_REQUIRED=true` refuses plaintext
//! everywhere except the exempt paths.
//!
//! The tunnel (`enforce_tunnel`) goes one step further and hides *which*
//! endpoint was called: every browser call is a `POST /x` whose sealed body
//! carries the real method, path and query string ahead of the real body. It
//! is unwrapped before CSRF and the routes run, so everything past it sees the
//! original request. Direct calls keep working alongside it.

use std::sync::Arc;

use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use axum::{
    body::{Body, to_bytes},
    extract::Request,
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use hkdf::Hkdf;
use p256::{PublicKey, SecretKey, ecdh::diffie_hellman, elliptic_curve::sec1::ToEncodedPoint};
use sha2::Sha256;

pub const KEY_HEADER: &str = "x-payload-key";
pub const ENCRYPTED_HEADER: &str = "x-payload-enc";
pub const TYPE_HEADER: &str = "x-payload-type";

const IV_LEN: usize = 12;
const TAG_LEN: usize = 16;
const HKDF_SALT: [u8; 32] = [0; 32];
const REQUEST_INFO: &[u8] = b"primelendrow payload v1 request";
const RESPONSE_INFO: &[u8] = b"primelendrow payload v1 response";

/// Ceilings on the *sealed* request body, enforced before decrypting. Each
/// route's own plaintext limit (routes::api_routes) still applies to the
/// opened body; these only stop a sealed blob being buffered far past what
/// its route could ever accept. `/kyc/submit` takes 24MB; every other route
/// that carries a body is capped at 16KB, well inside the 256KB default.
const KYC_SUBMIT_SEALED_MAX: usize = 24 * 1024 * 1024 + IV_LEN + TAG_LEN;
const DEFAULT_SEALED_MAX: usize = 256 * 1024 + IV_LEN + TAG_LEN;
/// Responses are JSON documents or short messages; a backstop, not a budget.
const RESPONSE_MAX: usize = 32 * 1024 * 1024;

/// Callers that are not our frontend and can never send a payload key.
const EXEMPT_PATHS: &[&str] = &[
    "/stripe/webhook",
    "/paypal/callback",
    "/stripe/return",
    "/stripe/refresh",
];

/// The one path a tunnelled call shows. Deliberately meaningless.
pub const TUNNEL_PATH: &str = "/x";
/// The inner method/path/content-type header of a tunnelled call. A path and
/// query string are short; this only bounds a malformed frame.
const TUNNEL_HEAD_MAX: usize = 8 * 1024;
/// The route isn't known until the frame is opened, so the tunnel has to
/// allow the largest body any route takes (KYC). The route's own plaintext
/// limit still applies once the request is unwrapped, and the rate and
/// concurrency limits run before the tunnel buffers anything.
const TUNNEL_SEALED_MAX: usize = KYC_SUBMIT_SEALED_MAX + 4 + TUNNEL_HEAD_MAX;

/// Marks a request the tunnel has already opened, so `enforce_payload`
/// doesn't try to open it again. A server-side extension: a client can't set it.
#[derive(Clone, Copy)]
struct Tunneled;

/// What a tunnelled call really was. Short keys: it rides in every request.
#[derive(serde::Deserialize)]
struct TunnelHead {
    /// Method.
    m: String,
    /// Path and query string, origin-form ("/loans/quote?amount=1").
    p: String,
    /// The body's content type, when there is a body.
    #[serde(default)]
    t: Option<String>,
}

#[derive(Clone)]
pub struct PayloadCipher {
    inner: Arc<Inner>,
}

struct Inner {
    secret: Option<SecretKey>,
    public_key: Option<String>,
    required: bool,
}

impl PayloadCipher {
    pub fn new(secret: Option<SecretKey>, required: bool) -> Self {
        let public_key = secret
            .as_ref()
            .map(|s| STANDARD.encode(s.public_key().to_encoded_point(false).as_bytes()));
        Self {
            inner: Arc::new(Inner { secret, public_key, required }),
        }
    }

    /// Reads `PAYLOAD_PRIVATE_KEY` (64 hex chars: a P-256 private scalar) and
    /// `PAYLOAD_ENCRYPTION_REQUIRED`. A key that is present but unusable is an
    /// error, not "off": the frontend would be sealing every request to a key
    /// this server can't open.
    pub fn from_env() -> Result<Self, &'static str> {
        let required = std::env::var("PAYLOAD_ENCRYPTION_REQUIRED")
            .map(|v| v.trim().eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let secret = match std::env::var("PAYLOAD_PRIVATE_KEY") {
            Ok(value) if !value.trim().is_empty() => {
                let bytes = hex::decode(value.trim())
                    .map_err(|_| "PAYLOAD_PRIVATE_KEY must be 64 hex characters")?;
                if bytes.len() != 32 {
                    return Err("PAYLOAD_PRIVATE_KEY must be 64 hex characters");
                }
                Some(
                    SecretKey::from_slice(&bytes)
                        .map_err(|_| "PAYLOAD_PRIVATE_KEY is not a valid P-256 private key")?,
                )
            }
            _ => None,
        };

        if required && secret.is_none() {
            return Err("PAYLOAD_ENCRYPTION_REQUIRED=true but PAYLOAD_PRIVATE_KEY is not set");
        }
        Ok(Self::new(secret, required))
    }

    pub fn is_required(&self) -> bool {
        self.inner.required
    }

    /// The public half, exactly as the frontend's `VITE_PAYLOAD_PUBLIC_KEY`
    /// must read — logged at boot so a mismatched pair is caught in one glance.
    pub fn public_key(&self) -> Option<&str> {
        self.inner.public_key.as_deref()
    }
}

/// Opens the request body before the handler sees it and seals the response
/// body after. Headers-only concerns (CSRF, rate limits, CORS) are untouched.
pub async fn enforce_payload(cipher: PayloadCipher, req: Request, next: Next) -> Response {
    // Opened by the tunnel already, and its response is sealed there.
    if req.method() == Method::OPTIONS || req.extensions().get::<Tunneled>().is_some() {
        return next.run(req).await;
    }
    let path = req.uri().path().to_owned();

    let Some(key_header) = req.headers().get(KEY_HEADER) else {
        if cipher.inner.required && !EXEMPT_PATHS.contains(&path.as_str()) {
            return reject(StatusCode::BAD_REQUEST, "Encrypted payload required");
        }
        return next.run(req).await;
    };

    let Some(secret) = cipher.inner.secret.as_ref() else {
        return reject(StatusCode::BAD_REQUEST, "Payload encryption is not enabled on this server");
    };
    let Some((request_key, response_key)) = request_keys(secret, key_header) else {
        return reject(StatusCode::BAD_REQUEST, "Invalid payload key");
    };

    let aad = format!("{} {}", req.method(), path);
    let (mut parts, body) = req.into_parts();

    let body = if parts.headers.get(ENCRYPTED_HEADER).is_some_and(|v| v == "1") {
        let limit = if path == "/kyc/submit" { KYC_SUBMIT_SEALED_MAX } else { DEFAULT_SEALED_MAX };
        let Ok(sealed) = to_bytes(body, limit).await else {
            return reject(StatusCode::PAYLOAD_TOO_LARGE, "Payload too large");
        };
        let Some(plain) = open(&request_key, aad.as_bytes(), &sealed) else {
            return reject(StatusCode::BAD_REQUEST, "Invalid encrypted payload");
        };
        let content_type = parts
            .headers
            .get(TYPE_HEADER)
            .cloned()
            .unwrap_or_else(|| HeaderValue::from_static("application/json"));
        parts.headers.insert(header::CONTENT_TYPE, content_type);
        parts.headers.insert(header::CONTENT_LENGTH, HeaderValue::from(plain.len()));
        Body::from(plain)
    } else {
        // A key with no encrypted body is a bodyless call (GET, logout) that
        // still wants its response sealed. A plaintext body riding beside a
        // key would slip past `required` with the flag simply stripped.
        if cipher.inner.required && has_body(&parts.headers) {
            return reject(StatusCode::BAD_REQUEST, "Encrypted payload required");
        }
        body
    };
    parts.headers.remove(KEY_HEADER);
    parts.headers.remove(ENCRYPTED_HEADER);
    parts.headers.remove(TYPE_HEADER);

    let response = next.run(Request::from_parts(parts, body)).await;
    seal_response(response, &response_key, &aad).await
}

/// Unwraps a tunnelled call (`POST /x`) back into the request it carries.
///
/// The sealed body is `u32 big-endian head length || head JSON || real body`,
/// opened under the AAD `"POST /x"`. The head names the real method and
/// path-and-query; the request is rebuilt with those, the real body and its
/// content type, and every original header and extension (cookies, the CSRF
/// token, the client's address) — then passed on, so CSRF, the route's body
/// limit, the handler and its logs all see the original call. The response
/// is sealed under `"POST /x <status>"`, the address the browser used.
///
/// Anything that isn't `/x` passes straight through untouched.
pub async fn enforce_tunnel(cipher: PayloadCipher, req: Request, next: Next) -> Response {
    if req.uri().path() != TUNNEL_PATH || req.method() == Method::OPTIONS {
        return next.run(req).await;
    }
    if req.method() != Method::POST {
        return reject(StatusCode::METHOD_NOT_ALLOWED, "Method not allowed");
    }
    let Some(secret) = cipher.inner.secret.as_ref() else {
        return reject(StatusCode::BAD_REQUEST, "Payload encryption is not enabled on this server");
    };
    let Some((request_key, response_key)) =
        req.headers().get(KEY_HEADER).and_then(|key_header| request_keys(secret, key_header))
    else {
        return reject(StatusCode::BAD_REQUEST, "Invalid payload key");
    };
    if !req.headers().get(ENCRYPTED_HEADER).is_some_and(|v| v == "1") {
        return reject(StatusCode::BAD_REQUEST, "Encrypted payload required");
    }

    let aad = format!("POST {TUNNEL_PATH}");
    let (mut parts, body) = req.into_parts();
    let Ok(sealed) = to_bytes(body, TUNNEL_SEALED_MAX).await else {
        return reject(StatusCode::PAYLOAD_TOO_LARGE, "Payload too large");
    };
    let Some(plain) = open(&request_key, aad.as_bytes(), &sealed) else {
        return reject(StatusCode::BAD_REQUEST, "Invalid encrypted payload");
    };
    let Some((head, inner_body)) = split_tunnel_frame(plain) else {
        return reject(StatusCode::BAD_REQUEST, "Invalid encrypted payload");
    };
    let Some((method, uri)) = tunnel_target(&head) else {
        return reject(StatusCode::BAD_REQUEST, "Invalid encrypted payload");
    };

    parts.method = method;
    parts.uri = uri;
    parts.headers.remove(KEY_HEADER);
    parts.headers.remove(ENCRYPTED_HEADER);
    parts.headers.remove(TYPE_HEADER);
    if inner_body.is_empty() {
        parts.headers.remove(header::CONTENT_TYPE);
        parts.headers.remove(header::CONTENT_LENGTH);
    } else {
        let content_type = head
            .t
            .as_deref()
            .and_then(|t| HeaderValue::from_str(t).ok())
            .unwrap_or_else(|| HeaderValue::from_static("application/json"));
        parts.headers.insert(header::CONTENT_TYPE, content_type);
        parts.headers.insert(header::CONTENT_LENGTH, HeaderValue::from(inner_body.len()));
    }
    parts.extensions.insert(Tunneled);

    let response = next.run(Request::from_parts(parts, Body::from(inner_body))).await;
    seal_response(response, &response_key, &aad).await
}

/// `u32 BE head length || head JSON || body` -> (head, body).
fn split_tunnel_frame(mut plain: Vec<u8>) -> Option<(TunnelHead, Vec<u8>)> {
    let len_bytes: [u8; 4] = plain.get(..4)?.try_into().ok()?;
    let head_len = u32::from_be_bytes(len_bytes) as usize;
    if head_len > TUNNEL_HEAD_MAX {
        return None;
    }
    let head: TunnelHead = serde_json::from_slice(plain.get(4..4 + head_len)?).ok()?;
    // In place rather than copied out: the body can be a 24MB KYC upload.
    plain.drain(..4 + head_len);
    Some((head, plain))
}

/// The real method and origin-form URI a tunnel head names — refusing
/// anything that isn't a plain API call: another host, the tunnel itself, or
/// the provider endpoints that are never called from the browser.
fn tunnel_target(head: &TunnelHead) -> Option<(Method, axum::http::Uri)> {
    let method = match head.m.as_str() {
        "GET" => Method::GET,
        "POST" => Method::POST,
        "PUT" => Method::PUT,
        "PATCH" => Method::PATCH,
        "DELETE" => Method::DELETE,
        _ => return None,
    };
    if !head.p.starts_with('/') || head.p.starts_with("//") {
        return None;
    }
    let uri: axum::http::Uri = head.p.parse().ok()?;
    if uri.scheme().is_some() || uri.authority().is_some() {
        return None;
    }
    if uri.path() == TUNNEL_PATH || EXEMPT_PATHS.contains(&uri.path()) {
        return None;
    }
    Some((method, uri))
}

/// ECDH against the client's one-off key in `x-payload-key`, then HKDF:
/// (request key, response key). None for a header that isn't a P-256 point.
fn request_keys(secret: &SecretKey, key_header: &HeaderValue) -> Option<([u8; 32], [u8; 32])> {
    key_header
        .to_str()
        .ok()
        .and_then(|v| STANDARD.decode(v.trim()).ok())
        .and_then(|epk| PublicKey::from_sec1_bytes(&epk).ok())
        .and_then(|epk| {
            let shared = diffie_hellman(secret.to_nonzero_scalar(), epk.as_affine());
            directional_keys(shared.raw_secret_bytes())
        })
}

async fn seal_response(response: Response, key: &[u8; 32], request_aad: &str) -> Response {
    let (mut parts, body) = response.into_parts();
    let bytes = match to_bytes(body, RESPONSE_MAX).await {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::error!("payload: could not buffer response for sealing: {e}");
            return reject(StatusCode::INTERNAL_SERVER_ERROR, "Response could not be encrypted");
        }
    };
    // Nothing to protect, and 204/304 may not carry a body at all.
    if bytes.is_empty() {
        return Response::from_parts(parts, Body::empty());
    }

    let aad = format!("{request_aad} {}", parts.status.as_u16());
    let Some(sealed) = seal(key, aad.as_bytes(), &bytes) else {
        tracing::error!("payload: sealing the response failed");
        return reject(StatusCode::INTERNAL_SERVER_ERROR, "Response could not be encrypted");
    };

    if let Some(content_type) = parts.headers.remove(header::CONTENT_TYPE) {
        parts.headers.insert(TYPE_HEADER, content_type);
    }
    parts
        .headers
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("application/octet-stream"));
    parts.headers.insert(ENCRYPTED_HEADER, HeaderValue::from_static("1"));
    parts.headers.remove(header::CONTENT_LENGTH);
    Response::from_parts(parts, Body::from(sealed))
}

/// HKDF-SHA256 over the ECDH shared secret: (request key, response key).
fn directional_keys(shared: &[u8]) -> Option<([u8; 32], [u8; 32])> {
    let hkdf = Hkdf::<Sha256>::new(Some(&HKDF_SALT), shared);
    let mut request_key = [0u8; 32];
    let mut response_key = [0u8; 32];
    hkdf.expand(REQUEST_INFO, &mut request_key).ok()?;
    hkdf.expand(RESPONSE_INFO, &mut response_key).ok()?;
    Some((request_key, response_key))
}

fn seal(key: &[u8; 32], aad: &[u8], plaintext: &[u8]) -> Option<Vec<u8>> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let iv: [u8; IV_LEN] = rand::random();
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&iv), Payload { msg: plaintext, aad })
        .ok()?;
    let mut out = Vec::with_capacity(IV_LEN + ciphertext.len());
    out.extend_from_slice(&iv);
    out.extend_from_slice(&ciphertext);
    Some(out)
}

fn open(key: &[u8; 32], aad: &[u8], sealed: &[u8]) -> Option<Vec<u8>> {
    if sealed.len() < IV_LEN + TAG_LEN {
        return None;
    }
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    cipher
        .decrypt(
            Nonce::from_slice(&sealed[..IV_LEN]),
            Payload { msg: &sealed[IV_LEN..], aad },
        )
        .ok()
}

fn has_body(headers: &HeaderMap) -> bool {
    headers.contains_key(header::TRANSFER_ENCODING)
        || headers
            .get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .is_some_and(|len| len > 0)
}

fn reject(status: StatusCode, message: &'static str) -> Response {
    (status, message).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Json, Router,
        body::Bytes,
        middleware,
        routing::{get, post},
    };
    use serde_json::{Value, json};
    use tower::ServiceExt;

    fn server(required: bool) -> PayloadCipher {
        PayloadCipher::new(Some(SecretKey::from_slice(&[7u8; 32]).unwrap()), required)
    }

    /// The browser's side of the exchange, done in Rust: (x-payload-key, request key, response key).
    fn client(server: &PayloadCipher, seed: u8) -> (String, [u8; 32], [u8; 32]) {
        let ephemeral = SecretKey::from_slice(&[seed; 32]).unwrap();
        let server_public =
            PublicKey::from_sec1_bytes(&STANDARD.decode(server.public_key().unwrap()).unwrap()).unwrap();
        let shared = diffie_hellman(ephemeral.to_nonzero_scalar(), server_public.as_affine());
        let (request_key, response_key) = directional_keys(shared.raw_secret_bytes()).unwrap();
        let epk = STANDARD.encode(ephemeral.public_key().to_encoded_point(false).as_bytes());
        (epk, request_key, response_key)
    }

    fn app(cipher: PayloadCipher) -> Router {
        Router::new()
            .route(
                "/echo",
                post(|headers: HeaderMap, body: Bytes| async move {
                    let content_type = headers
                        .get(header::CONTENT_TYPE)
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or_default()
                        .to_owned();
                    (
                        StatusCode::CREATED,
                        Json(json!({
                            "content_type": content_type,
                            "body": String::from_utf8(body.to_vec()).unwrap(),
                        })),
                    )
                }),
            )
            .route("/hello", get(|| async { Json(json!({ "hello": "world" })) }))
            .route(
                "/where",
                get(|uri: axum::http::Uri| async move {
                    Json(json!({ "path": uri.path(), "query": uri.query() }))
                }),
            )
            .route("/stripe/webhook", post(|| async { "ok" }))
            .route("/logout", post(|| async { StatusCode::NO_CONTENT }))
            .layer(middleware::from_fn(move |req, next| {
                enforce_payload(cipher.clone(), req, next)
            }))
    }

    fn sealed_post(path: &str, epk: &str, request_key: &[u8; 32], aad: &str, body: &[u8]) -> Request {
        Request::post(path)
            .header(KEY_HEADER, epk)
            .header(ENCRYPTED_HEADER, "1")
            .header(TYPE_HEADER, "application/json")
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .body(Body::from(seal(request_key, aad.as_bytes(), body).unwrap()))
            .unwrap()
    }

    async fn body_bytes(response: Response) -> Vec<u8> {
        to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()
    }

    #[tokio::test]
    async fn encrypted_post_reaches_handler_as_json_and_response_is_sealed() {
        let cipher = server(true);
        let (epk, request_key, response_key) = client(&cipher, 3);
        let request = sealed_post("/echo", &epk, &request_key, "POST /echo", br#"{"amount":1500}"#);

        let response = app(cipher).oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(response.headers()[ENCRYPTED_HEADER], "1");
        assert_eq!(response.headers()[TYPE_HEADER], "application/json");
        assert_eq!(response.headers()[header::CONTENT_TYPE], "application/octet-stream");

        let sealed = body_bytes(response).await;
        let plain = open(&response_key, b"POST /echo 201", &sealed).expect("response opens under its status");
        let echoed: Value = serde_json::from_slice(&plain).unwrap();
        assert_eq!(echoed["content_type"], "application/json");
        assert_eq!(echoed["body"], r#"{"amount":1500}"#);
    }

    #[tokio::test]
    async fn bodyless_request_gets_a_sealed_response() {
        let cipher = server(true);
        let (epk, _, response_key) = client(&cipher, 4);
        let request = Request::get("/hello").header(KEY_HEADER, &epk).body(Body::empty()).unwrap();

        let response = app(cipher).oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let plain = open(&response_key, b"GET /hello 200", &body_bytes(response).await).unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&plain).unwrap(), json!({ "hello": "world" }));
    }

    #[tokio::test]
    async fn response_status_is_authenticated() {
        let cipher = server(false);
        let (epk, _, response_key) = client(&cipher, 5);
        let request = Request::get("/hello").header(KEY_HEADER, &epk).body(Body::empty()).unwrap();
        let sealed = body_bytes(app(cipher).oneshot(request).await.unwrap()).await;
        assert!(open(&response_key, b"GET /hello 500", &sealed).is_none());
    }

    #[tokio::test]
    async fn plaintext_still_works_while_not_required() {
        let request = Request::post("/echo")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"page":1}"#))
            .unwrap();
        let response = app(server(false)).oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        assert!(response.headers().get(ENCRYPTED_HEADER).is_none());
        let echoed: Value = serde_json::from_slice(&body_bytes(response).await).unwrap();
        assert_eq!(echoed["body"], r#"{"page":1}"#);
    }

    #[tokio::test]
    async fn required_refuses_plaintext_but_not_exempt_paths() {
        let refused = app(server(true))
            .oneshot(Request::get("/hello").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(refused.status(), StatusCode::BAD_REQUEST);

        let webhook = app(server(true))
            .oneshot(Request::post("/stripe/webhook").body(Body::from("{}")).unwrap())
            .await
            .unwrap();
        assert_eq!(webhook.status(), StatusCode::OK);
        assert_eq!(body_bytes(webhook).await, b"ok");
    }

    #[tokio::test]
    async fn required_refuses_a_plaintext_body_beside_a_key() {
        let cipher = server(true);
        let (epk, _, _) = client(&cipher, 6);
        let request = Request::post("/echo")
            .header(KEY_HEADER, &epk)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::CONTENT_LENGTH, "10")
            .body(Body::from(r#"{"page":1}"#))
            .unwrap();
        let response = app(cipher).oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn ciphertext_is_bound_to_its_endpoint() {
        let cipher = server(false);
        let (epk, request_key, _) = client(&cipher, 8);
        let request = sealed_post("/echo", &epk, &request_key, "POST /pool/withdraw", b"{}");
        let response = app(cipher).oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn tampered_ciphertext_is_refused() {
        let cipher = server(false);
        let (epk, request_key, _) = client(&cipher, 9);
        let mut sealed = seal(&request_key, b"POST /echo", b"{}").unwrap();
        let last = sealed.len() - 1;
        sealed[last] ^= 1;
        let request = Request::post("/echo")
            .header(KEY_HEADER, &epk)
            .header(ENCRYPTED_HEADER, "1")
            .body(Body::from(sealed))
            .unwrap();
        let response = app(cipher).oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn request_key_cannot_open_a_response() {
        let cipher = server(false);
        let (epk, request_key, _) = client(&cipher, 10);
        let request = Request::get("/hello").header(KEY_HEADER, &epk).body(Body::empty()).unwrap();
        let sealed = body_bytes(app(cipher).oneshot(request).await.unwrap()).await;
        assert!(open(&request_key, b"GET /hello 200", &sealed).is_none());
    }

    #[tokio::test]
    async fn empty_responses_pass_through_unsealed() {
        let cipher = server(true);
        let (epk, _, _) = client(&cipher, 11);
        let request = Request::post("/logout").header(KEY_HEADER, &epk).body(Body::empty()).unwrap();
        let response = app(cipher).oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(response.headers().get(ENCRYPTED_HEADER).is_none());
    }

    /// The engine's shape: the tunnel on an outer router whose fallback is the
    /// API router, so the rewritten path is what gets routed.
    fn tunnelled_app(cipher: PayloadCipher) -> Router {
        let tunnel = cipher.clone();
        Router::new()
            .fallback_service(app(cipher))
            .layer(middleware::from_fn(move |req, next| enforce_tunnel(tunnel.clone(), req, next)))
    }

    fn frame(head: Value, body: &[u8]) -> Vec<u8> {
        let head = serde_json::to_vec(&head).unwrap();
        let mut out = (head.len() as u32).to_be_bytes().to_vec();
        out.extend_from_slice(&head);
        out.extend_from_slice(body);
        out
    }

    fn tunnel_request(epk: &str, request_key: &[u8; 32], aad: &str, plain: &[u8]) -> Request {
        Request::post(TUNNEL_PATH)
            .header(KEY_HEADER, epk)
            .header(ENCRYPTED_HEADER, "1")
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .body(Body::from(seal(request_key, aad.as_bytes(), plain).unwrap()))
            .unwrap()
    }

    #[tokio::test]
    async fn tunnel_carries_a_get_with_its_query_string() {
        let cipher = server(true);
        let (epk, request_key, response_key) = client(&cipher, 20);
        let plain = frame(json!({ "m": "GET", "p": "/where?page=2&status=active" }), b"");
        let request = tunnel_request(&epk, &request_key, "POST /x", &plain);

        let response = tunnelled_app(cipher).oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let plain = open(&response_key, b"POST /x 200", &body_bytes(response).await)
            .expect("the response is sealed to the address the browser used");
        let seen: Value = serde_json::from_slice(&plain).unwrap();
        assert_eq!(seen, json!({ "path": "/where", "query": "page=2&status=active" }));
    }

    #[tokio::test]
    async fn tunnel_carries_a_post_body_and_its_content_type() {
        let cipher = server(true);
        let (epk, request_key, response_key) = client(&cipher, 21);
        let plain = frame(json!({ "m": "POST", "p": "/echo", "t": "application/json" }), br#"{"amount":1500}"#);
        let request = tunnel_request(&epk, &request_key, "POST /x", &plain);

        let response = tunnelled_app(cipher).oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let plain = open(&response_key, b"POST /x 201", &body_bytes(response).await).unwrap();
        let echoed: Value = serde_json::from_slice(&plain).unwrap();
        assert_eq!(echoed["content_type"], "application/json");
        assert_eq!(echoed["body"], r#"{"amount":1500}"#);
    }

    #[tokio::test]
    async fn tunnel_keeps_the_real_method() {
        let cipher = server(true);
        let (epk, request_key, _) = client(&cipher, 22);
        // /hello is GET-only: a POST carried inside must still be refused.
        let plain = frame(json!({ "m": "POST", "p": "/hello" }), b"");
        let response = tunnelled_app(cipher)
            .oneshot(tunnel_request(&epk, &request_key, "POST /x", &plain))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    async fn tunnel_refuses_plaintext_and_foreign_ciphertext() {
        let cipher = server(false);
        let plaintext = Request::post(TUNNEL_PATH)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"m":"GET","p":"/hello"}"#))
            .unwrap();
        let response = tunnelled_app(cipher.clone()).oneshot(plaintext).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        // Sealed for a different address than the one it was sent to.
        let (epk, request_key, _) = client(&cipher, 23);
        let plain = frame(json!({ "m": "GET", "p": "/hello" }), b"");
        let response = tunnelled_app(cipher)
            .oneshot(tunnel_request(&epk, &request_key, "POST /hello", &plain))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn tunnel_refuses_targets_that_are_not_api_calls() {
        let cipher = server(true);
        for (seed, path) in [
            (24u8, "/stripe/webhook"),
            (25, "/x"),
            (26, "//evil.example/hello"),
            (27, "https://evil.example/hello"),
            (28, "hello"),
        ] {
            let (epk, request_key, _) = client(&cipher, seed);
            let plain = frame(json!({ "m": "POST", "p": path }), b"{}");
            let response = tunnelled_app(cipher.clone())
                .oneshot(tunnel_request(&epk, &request_key, "POST /x", &plain))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{path}");
        }
    }

    #[tokio::test]
    async fn tunnel_refuses_a_malformed_frame() {
        let cipher = server(true);
        let (epk, request_key, _) = client(&cipher, 29);
        let mut plain = frame(json!({ "m": "GET", "p": "/hello" }), b"");
        plain[..4].copy_from_slice(&u32::MAX.to_be_bytes());
        let response = tunnelled_app(cipher)
            .oneshot(tunnel_request(&epk, &request_key, "POST /x", &plain))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn direct_calls_still_work_beside_the_tunnel() {
        let cipher = server(true);
        let (epk, _, response_key) = client(&cipher, 30);
        let request = Request::get("/hello").header(KEY_HEADER, &epk).body(Body::empty()).unwrap();
        let response = tunnelled_app(cipher).oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(open(&response_key, b"GET /hello 200", &body_bytes(response).await).is_some());
    }

    #[tokio::test]
    async fn a_key_is_refused_when_the_server_has_none() {
        let cipher = server(false);
        let (epk, _, _) = client(&cipher, 12);
        let request = Request::get("/hello").header(KEY_HEADER, &epk).body(Body::empty()).unwrap();
        let response = app(PayloadCipher::new(None, false)).oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
