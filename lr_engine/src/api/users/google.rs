//! GET /auth/google/start and GET /auth/google/callback — sign in with Google.
//!
//! OpenID Connect's authorization code flow, run entirely on the server so the
//! client secret never reaches a browser, with every protection the spec and
//! Google's guidance offer:
//!
//!   * **PKCE (S256).** The code Google returns is useless without the
//!     verifier, which never leaves this server.
//!   * **state, bound to the browser.** A random value stored (hashed) in
//!     `oauth_states` AND set in a `__Host-` cookie. The callback must present
//!     both, so a callback link planted in someone else's browser — login
//!     CSRF, signing the victim into the attacker's account — has no cookie
//!     and is refused. Deleted on first use, expired after ten minutes.
//!   * **nonce.** Stored with the state and required back inside the ID token,
//!     so a token minted for some other sign-in can't be replayed into this one.
//!   * **Exact redirect URI** from GOOGLE_REDIRECT_URI, never derived from the
//!     request's Host header.
//!   * **ID token checks:** issuer, audience (our client id), expiry, nonce, and
//!     `email_verified`. The token comes straight from Google's token endpoint
//!     over TLS in a server-to-server call, which OpenID Connect Core §3.1.3.7
//!     accepts in place of checking the token's signature — so no JWT library
//!     and no key-fetching code to get wrong.
//!   * **Fixed landing pages.** Success and failure both redirect to paths on
//!     CLIENT_URL's origin; nothing in the request chooses where the browser
//!     goes next, so there is no open redirect.
//!
//! What a successful callback does is exactly what a password login does:
//! every other session of that member is ended, one new session is opened,
//! and the same `__Host-` session and csrf cookies are set.
//!
//! **Which account.** Identities are keyed by Google's `sub`, never by email,
//! as Google advises. A Google account already linked signs straight in. One
//! not yet linked, whose verified email matches a member's, is linked to that
//! member ONLY where Google is authoritative for the address — Gmail, or a
//! Workspace account (`hd` set) — since only then can Google vouch the person
//! owns it today; anyone else is asked to log in with their password. With no
//! match, the login button refuses (the member is sent to sign up), and the
//! sign-up button creates a Pending account — KYC still applies.
//!
//! Google's own tokens are used once, to read the ID token, and never stored.
//!
//! Env: GOOGLE_CLIENT_ID, GOOGLE_CLIENT_SECRET, GOOGLE_REDIRECT_URI (this
//! engine's public `/auth/google/callback` URL, exactly as registered with
//! Google), CLIENT_URL.

use argon2::{
    Algorithm, Argon2, Params, PasswordHasher, Version,
    password_hash::{SaltString, rand_core::OsRng},
};
use axum::{
    Extension,
    extract::Query,
    http::{HeaderMap, HeaderValue, StatusCode, header::{LOCATION, SET_COOKIE}},
    response::{IntoResponse, Response},
};
use base64::Engine as _;
use chrono::Utc;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use subtle::ConstantTimeEq;
use uuid::Uuid;

use super::shared::{
    SESSION_MAX_AGE, clear_legacy_domain_csrf_cookie, clear_oauth_state_cookie, csrf_cookie,
    extract_cookie_value, hash_permit, is_valid_email, new_csrf_token, oauth_state_cookie,
    oauth_state_cookie_name, session_cookie,
};

const AUTHORIZE_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
/// How long a sign-in attempt may take, Google's screens included.
const STATE_TTL: i64 = 600;

struct GoogleConfig {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
}

fn config() -> Option<GoogleConfig> {
    let read = |name: &str| std::env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    Some(GoogleConfig {
        client_id: read("GOOGLE_CLIENT_ID")?,
        client_secret: read("GOOGLE_CLIENT_SECRET")?,
        redirect_uri: read("GOOGLE_REDIRECT_URI")?,
    })
}

/// The app's origin, CLIENT_URL's first entry — the same rule the PayPal and
/// Stripe redirects follow, since a redirect can only go to one place.
fn client_origin() -> String {
    std::env::var("CLIENT_URL")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .find(|s| !s.is_empty())
        .unwrap_or("http://localhost:5173")
        .trim_end_matches('/')
        .to_string()
}

/// A random value from the OS's CSPRNG, as hex — the same source the session
/// ids and csrf tokens use. Two UUIDs give 244 random bits.
fn random_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn sha256_hex(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

fn redirect(to: &str, cookies: Vec<HeaderValue>) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(LOCATION, HeaderValue::from_str(to).unwrap_or(HeaderValue::from_static("/")));
    for cookie in cookies {
        headers.append(SET_COOKIE, cookie);
    }
    // No caching of a page that sets session cookies.
    headers.insert(axum::http::header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    (StatusCode::SEE_OTHER, headers).into_response()
}

/// Back to the sign-in page with a reason code the page turns into a message.
/// Codes, never free text, so nothing from Google or the request is echoed.
fn fail(reason: &'static str) -> Response {
    redirect(&format!("{}/auth?oauth_error={reason}", client_origin()), vec![clear_oauth_state_cookie()])
}

#[derive(Deserialize)]
pub struct StartQuery {
    #[serde(default)]
    intent: String,
}

pub async fn start(Extension(pool): Extension<PgPool>, Query(q): Query<StartQuery>) -> Response {
    let Some(cfg) = config() else {
        return fail("unavailable");
    };
    let intent = if q.intent == "register" { "register" } else { "login" };

    let now = Utc::now().timestamp();
    let state = random_token();
    let code_verifier = random_token(); // 64 hex characters, inside PKCE's 43–128
    let nonce = Uuid::new_v4().simple().to_string();
    let code_challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(code_verifier.as_bytes()));

    // Housekeeping first: abandoned attempts don't accumulate.
    let _ = sqlx::query("DELETE FROM public.oauth_states WHERE expires_at <= $1")
        .bind(now)
        .execute(&pool)
        .await;
    if let Err(e) = sqlx::query(
        "INSERT INTO public.oauth_states (state_hash, provider, intent, code_verifier, nonce, created_at, expires_at)
         VALUES ($1, 'google', $2, $3, $4, $5, $6)",
    )
    .bind(sha256_hex(&state))
    .bind(intent)
    .bind(&code_verifier)
    .bind(&nonce)
    .bind(now)
    .bind(now + STATE_TTL)
    .execute(&pool)
    .await
    {
        tracing::error!("DB oauth state: {e}");
        return fail("failed");
    }

    let url = match reqwest::Url::parse_with_params(
        AUTHORIZE_URL,
        &[
            ("client_id", cfg.client_id.as_str()),
            ("redirect_uri", cfg.redirect_uri.as_str()),
            ("response_type", "code"),
            ("scope", "openid email profile"),
            ("state", state.as_str()),
            ("nonce", nonce.as_str()),
            ("code_challenge", code_challenge.as_str()),
            ("code_challenge_method", "S256"),
            // Always let the member choose, so a shared device never signs
            // someone in as whoever used Google on it last.
            ("prompt", "select_account"),
        ],
    ) {
        Ok(url) => url,
        Err(_) => return fail("failed"),
    };

    redirect(url.as_str(), vec![oauth_state_cookie(&state)])
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct TokenResponse {
    id_token: Option<String>,
}

/// The ID token claims this flow relies on.
#[derive(Deserialize)]
struct Claims {
    iss: String,
    aud: String,
    sub: String,
    exp: i64,
    #[serde(default)]
    iat: i64,
    #[serde(default)]
    nonce: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    email_verified: bool,
    /// Set only for Google Workspace accounts: the domain Google hosts.
    #[serde(default)]
    hd: String,
    #[serde(default)]
    name: String,
}

/// Whether Google can vouch that this person owns the address right now.
/// Google's own rule: a Gmail address, or a Workspace account (`hd` set). Any
/// other address Google verified once, possibly long ago, and its guidance is
/// to fall back to a password rather than trust it.
fn google_is_authoritative(email: &str, hd: &str) -> bool {
    email.ends_with("@gmail.com") || !hd.is_empty()
}

/// Reads the ID token's claims. The signature is not checked, by design: the
/// token arrived directly from Google's token endpoint over TLS (see the module
/// comment); the claims are then checked field by field by the caller.
fn read_claims(id_token: &str) -> Option<Claims> {
    let payload = id_token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// A username for a new Google account: the member's name (or their email's
/// local part) reduced to letters, digits and underscores, with a short random
/// suffix whenever it would collide with an existing username — usernames are
/// how guarantors are found, so two members must never share one.
async fn free_username(pool: &PgPool, name: &str, email: &str) -> Result<String, sqlx::Error> {
    let source = if name.trim().is_empty() { email.split('@').next().unwrap_or("") } else { name };
    let mut base: String = source
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    base.truncate(24);
    if base.len() < 3 {
        base = "member".to_string();
    }

    for attempt in 0..6 {
        let candidate = if attempt == 0 {
            base.clone()
        } else {
            format!("{base}_{}", &Uuid::new_v4().simple().to_string()[..4])
        };
        let taken: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM public.users WHERE lower(username) = lower($1))")
            .bind(&candidate)
            .fetch_one(pool)
            .await?;
        if !taken {
            return Ok(candidate);
        }
    }
    Ok(format!("{base}_{}", Uuid::new_v4().simple()))
}

/// A password hash nobody knows the password to. Google accounts still need a
/// row in `password_hash`, and a real Argon2 hash of a random secret keeps the
/// password login behaving exactly as it does for a wrong password — no error,
/// no tell that the account is Google-only. The member can set a real password
/// any time through "Forgot password", which is also how they'd sign in if
/// they ever lose the Google account.
async fn unusable_password_hash() -> Option<String> {
    let secret = random_token();
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, Params::new(65536, 3, 4, None).ok()?);
    let permit = hash_permit().await;
    let hash = argon2.hash_password(secret.as_bytes(), &SaltString::generate(&mut OsRng)).ok()?.to_string();
    drop(permit);
    Some(hash)
}

pub async fn callback(
    Extension(pool): Extension<PgPool>,
    request_headers: HeaderMap,
    Query(q): Query<CallbackQuery>,
) -> Response {
    let Some(cfg) = config() else {
        return fail("unavailable");
    };
    // The member pressed Cancel on Google's screen, or Google refused.
    if q.error.is_some() {
        return fail("cancelled");
    }
    let (Some(code), Some(state)) = (q.code, q.state) else {
        return fail("failed");
    };

    // The state must match the cookie set when this browser started the
    // attempt. Constant-time, though the value is random either way.
    let cookie_state = extract_cookie_value(&request_headers, oauth_state_cookie_name()).unwrap_or_default();
    if cookie_state.is_empty() || !bool::from(cookie_state.as_bytes().ct_eq(state.as_bytes())) {
        return fail("expired");
    }

    let now = Utc::now().timestamp();
    // Deleted as it is read: a state is good for exactly one callback.
    let attempt: Option<(String, String, String, i64)> = match sqlx::query_as(
        "DELETE FROM public.oauth_states WHERE state_hash = $1 AND provider = 'google'
         RETURNING intent, code_verifier, nonce, expires_at",
    )
    .bind(sha256_hex(&state))
    .fetch_optional(&pool)
    .await
    {
        Ok(row) => row,
        Err(e) => {
            tracing::error!("DB oauth state read: {e}");
            return fail("failed");
        }
    };
    let Some((intent, code_verifier, nonce, expires_at)) = attempt else {
        return fail("expired");
    };
    if expires_at <= now {
        return fail("expired");
    }

    // Code for tokens, server to server. The body is built with the same
    // encoder a query string uses, which is what form encoding is.
    let body = match reqwest::Url::parse_with_params(
        "http://form.invalid/",
        &[
            ("code", code.as_str()),
            ("client_id", cfg.client_id.as_str()),
            ("client_secret", cfg.client_secret.as_str()),
            ("redirect_uri", cfg.redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
            ("code_verifier", code_verifier.as_str()),
        ],
    ) {
        Ok(url) => url.query().unwrap_or_default().to_string(),
        Err(_) => return fail("failed"),
    };
    let tokens: TokenResponse = match reqwest::Client::new()
        .post(TOKEN_URL)
        .header(axum::http::header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(body)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
    {
        Ok(res) if res.status().is_success() => match res.json().await {
            Ok(tokens) => tokens,
            Err(e) => {
                tracing::error!("google token body: {e}");
                return fail("failed");
            }
        },
        Ok(res) => {
            tracing::warn!(status = %res.status(), "google token exchange refused");
            return fail("failed");
        }
        Err(e) => {
            tracing::error!("google token exchange: {e}");
            return fail("failed");
        }
    };

    let Some(claims) = tokens.id_token.as_deref().and_then(read_claims) else {
        return fail("failed");
    };
    let issuer_ok = claims.iss == "https://accounts.google.com" || claims.iss == "accounts.google.com";
    let nonce_ok = bool::from(claims.nonce.as_bytes().ct_eq(nonce.as_bytes()));
    if !issuer_ok
        || claims.aud != cfg.client_id
        || claims.exp <= now
        || claims.iat > now + 300
        || !nonce_ok
        || claims.sub.is_empty()
        || claims.sub.len() > 255
    {
        tracing::warn!("google id token failed validation");
        return fail("failed");
    }
    let email = claims.email.trim().to_ascii_lowercase();
    if !claims.email_verified || !is_valid_email(&email) || email.len() > 255 {
        return fail("email_unverified");
    }

    // Who this is: a linked Google account, else a member with the same
    // verified email, else — only from the sign-up button — a new member.
    let linked: Option<Uuid> = match sqlx::query_scalar(
        "SELECT user_id FROM public.user_identities WHERE provider = 'google' AND subject = $1",
    )
    .bind(&claims.sub)
    .fetch_optional(&pool)
    .await
    {
        Ok(found) => found,
        Err(e) => {
            tracing::error!("DB identity lookup: {e}");
            return fail("failed");
        }
    };

    let by_email: Option<Uuid> = if linked.is_some() {
        None
    } else {
        match sqlx::query_scalar("SELECT id FROM public.users WHERE email = $1")
            .bind(&email)
            .fetch_optional(&pool)
            .await
        {
            Ok(found) => found,
            Err(e) => {
                tracing::error!("DB user by email: {e}");
                return fail("failed");
            }
        }
    };

    if linked.is_none() && by_email.is_none() && intent != "register" {
        return fail("no_account");
    }
    // Linking a Google account to an EXISTING member by email is only safe
    // where Google is authoritative for that email. Otherwise whoever holds a
    // Google account once verified for the address could walk into the
    // member's account, so the member logs in with their password instead.
    if linked.is_none() && by_email.is_some() && !google_is_authoritative(&email, &claims.hd) {
        return fail("link_password");
    }

    // A new account is prepared outside the transaction: the username needs a
    // few lookups and the password hash a hashing slot, neither of which
    // should hold a transaction open.
    let new_account = if linked.is_none() && by_email.is_none() {
        let username = match free_username(&pool, &claims.name, &email).await {
            Ok(username) => username,
            Err(e) => {
                tracing::error!("DB username check: {e}");
                return fail("failed");
            }
        };
        let Some(password_hash) = unusable_password_hash().await else {
            return fail("failed");
        };
        Some((username, password_hash))
    } else {
        None
    };

    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!("DB begin google sign-in: {e}");
            return fail("failed");
        }
    };

    let user_id = match (linked, by_email, new_account) {
        (Some(user_id), _, _) => user_id,
        (None, Some(user_id), _) => user_id,
        (None, None, Some((username, password_hash))) => {
            let user_id = Uuid::new_v4();
            // Pending, like every new member: KYC decides the rest. The
            // credit-score row comes from the users trigger (015).
            if let Err(e) = sqlx::query(
                "INSERT INTO public.users (id, username, email, password_hash, role, created_at, updated_at)
                 VALUES ($1, $2, $3, $4, 'Pending', $5, $5)",
            )
            .bind(user_id)
            .bind(&username)
            .bind(&email)
            .bind(&password_hash)
            .bind(now)
            .execute(&mut *tx)
            .await
            {
                // The one expected cause: the same email registered in the
                // moments since the lookup above. Try again and it links.
                tracing::warn!("google sign-up insert: {e}");
                return fail("failed");
            }
            tracing::info!(%user_id, "user registered with google");
            user_id
        }
        (None, None, None) => return fail("failed"),
    };

    // Link (or refresh) the identity, and insist it lands on THIS member.
    //   * The same Google account linked to a different member (possible only
    //     if two sign-ins race): the conflict's WHERE matches nothing, no row
    //     is written, and the sign-in fails rather than proceed as the wrong
    //     person.
    //   * A member who already linked a different Google account: the unique
    //     key on (user_id, provider) refuses a second one, so another Google
    //     account showing the same email can't take the account over.
    match sqlx::query(
        "INSERT INTO public.user_identities (user_id, provider, subject, email, created_at, last_login_at)
         VALUES ($1, 'google', $2, $3, $4, $4)
         ON CONFLICT (provider, subject) DO UPDATE
            SET email = EXCLUDED.email, last_login_at = EXCLUDED.last_login_at
          WHERE public.user_identities.user_id = EXCLUDED.user_id",
    )
    .bind(user_id)
    .bind(&claims.sub)
    .bind(&email)
    .bind(now)
    .execute(&mut *tx)
    .await
    {
        Ok(done) if done.rows_affected() == 1 => {}
        Ok(_) => {
            tracing::warn!(%user_id, "google account is linked to a different member");
            return fail("failed");
        }
        Err(e) => {
            tracing::warn!(%user_id, "google identity link refused: {e}");
            return fail("failed");
        }
    }

    // One session per member, exactly as the password login does it.
    if let Err(e) = sqlx::query("DELETE FROM public.sessions WHERE user_id = $1 OR expires_at <= $2")
        .bind(user_id)
        .bind(now)
        .execute(&mut *tx)
        .await
    {
        tracing::error!("DB session cleanup: {e}");
        return fail("failed");
    }
    let session_id = Uuid::new_v4();
    if let Err(e) = sqlx::query("INSERT INTO public.sessions (id, user_id, created_at, expires_at) VALUES ($1, $2, $3, $4)")
        .bind(session_id)
        .bind(user_id)
        .bind(now)
        .bind(now + SESSION_MAX_AGE)
        .execute(&mut *tx)
        .await
    {
        tracing::error!("DB session create: {e}");
        return fail("failed");
    }
    if let Err(e) = tx.commit().await {
        tracing::error!("DB commit google sign-in: {e}");
        return fail("failed");
    }

    tracing::info!(%user_id, "user signed in with google");

    let mut cookies = vec![session_cookie(session_id), csrf_cookie(&new_csrf_token()), clear_oauth_state_cookie()];
    if let Some(clear) = clear_legacy_domain_csrf_cookie() {
        cookies.push(clear);
    }
    // The app picks the session up on load (GET /auth/session), which also
    // hands it the csrf token from the cookie just set.
    redirect(&format!("{}/dashboard", client_origin()), cookies)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_claims_of_an_id_token() {
        let claims = serde_json::json!({
            "iss": "https://accounts.google.com", "aud": "client.apps.googleusercontent.com",
            "sub": "1234567890", "exp": 2_000_000_000i64, "iat": 1_900_000_000i64, "nonce": "n",
            "email": "Member@Example.com", "email_verified": true, "name": "A Member",
        });
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string());
        let parsed = read_claims(&format!("header.{payload}.signature")).expect("claims");
        assert_eq!(parsed.sub, "1234567890");
        assert!(parsed.email_verified);
        assert_eq!(parsed.nonce, "n");
        assert!(read_claims("not-a-token").is_none());
    }

    #[test]
    fn only_gmail_and_workspace_are_trusted_for_linking() {
        assert!(google_is_authoritative("member@gmail.com", ""));
        assert!(google_is_authoritative("member@company.ph", "company.ph"));
        // A Google account made with some other address: verified once, but
        // Google can't vouch for it now.
        assert!(!google_is_authoritative("member@yahoo.com", ""));
        assert!(!google_is_authoritative("member@gmail.com.evil.test", ""));
    }

    #[test]
    fn pkce_challenge_is_the_rfc_7636_example() {
        // RFC 7636 Appendix B: this verifier must produce this S256 challenge.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }
}
