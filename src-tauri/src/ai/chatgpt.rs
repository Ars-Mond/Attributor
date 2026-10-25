//! ChatGPT subscription access through OpenAI's "Sign in with ChatGPT" plan usage for open-source, locally run apps (developers.openai.com/siwc/token-sharing-open-source).
//!
//! Sign-in is an OAuth authorization-code flow with PKCE, an OIDC nonce and a loopback redirect on `127.0.0.1`. The first sign-in registers the app through the reserved `dynamic_agent_client`; the issued per-user client id and the installation's `ext_agent_host_id` are kept (they are not secret) and reused afterwards. Tokens are sealed on disk (see [`super::secrets`]).
//! Inference goes to the public Responses API with the plan token; the plan route requires `stream: true` and `store: false`.

use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use futures_util::StreamExt;
use ring::rand::{SecureRandom, SystemRandom};
use ring::signature::{RsaPublicKeyComponents, RSA_PKCS1_2048_8192_SHA256};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::http;
use super::image::ImagePayload;
use super::openai;
use super::secrets;
use super::types::{AttributionConfig, ChatGptAccount, ModelInfo};

const ISSUER: &str = "https://auth.openai.com";
const AUTHORIZE_URL: &str = "https://auth.openai.com/api/accounts/authorize";
const TOKEN_URL: &str = "https://auth.openai.com/api/accounts/oauth/token";
const REVOKE_URL: &str = "https://auth.openai.com/api/accounts/oauth/revoke";
const JWKS_URL: &str = "https://auth.openai.com/.well-known/jwks.json";
/// Reserved public client id used only for the first, registering sign-in.
const REGISTRATION_CLIENT_ID: &str = "dynamic_agent_client";
/// Audience of plan tokens; sent as `resource` on every authorization and token request.
const RESOURCE: &str = "https://api.openai.com/v1";
const API_BASE: &str = "https://api.openai.com/v1";
/// Identity scopes plus the plan-usage scopes.
const SCOPE: &str = "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";
/// Granted only when the user agreed to share plan usage with the app.
const PLAN_SCOPE: &str = "chatgpt.tokens.use.direct";
const AGENT_NAME: &str = "Attributor";
const CALLBACK_PATH: &str = "/auth/callback";
/// How long sign-in waits for the browser redirect.
const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);
/// Per-connection read timeout on the loopback listener; browsers open speculative connections that never send a request.
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);
/// Access tokens are refreshed this long before they expire.
const REFRESH_MARGIN_SECS: u64 = 300;
/// Accepted clock difference when checking ID-token expiry.
const CLOCK_LEEWAY_SECS: u64 = 60;
/// Sealed file holding the tokens.
const VAULT_NAME: &str = "chatgpt";
/// Plain file holding the non-secret app registration.
const REGISTRATION_FILE: &str = "chatgpt-client.json";
/// Offered when the model catalog cannot be read (it is known to omit models at times).
const FALLBACK_MODELS: &[&str] = &[
    "gpt-6.1-sol",
    "gpt-6-astra",
    "gpt-6-sol",
    "gpt-6-luna",
    "gpt-5.6-sol",
    "gpt-5.6-terra",
    "gpt-5.6-luna",
];

/// The app's registration with OpenAI: a stable per-installation host id and the client id issued to the signed-in account.
#[derive(Serialize, Deserialize, Default, Clone)]
struct Registration {
    host_id: String,
    client_id: Option<String>,
    /// ID-token subject of the account `client_id` was issued to.
    #[serde(default)]
    subject: Option<String>,
    /// Set after the user declined plan sharing, so the next sign-in shows the consent screen again.
    #[serde(default)]
    needs_consent: bool,
}

/// The signed-in user's tokens. `expires_at` is the access token's expiry in Unix seconds.
#[derive(Serialize, Deserialize, Clone)]
struct Tokens {
    client_id: String,
    access_token: String,
    refresh_token: String,
    expires_at: u64,
    email: Option<String>,
}

/// Token endpoint response (authorization-code and refresh grants).
#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    scope: Option<String>,
}

/// Cached tokens: `None` until loaded from disk, `Some(None)` when signed out.
type TokenSlot = Option<Option<Tokens>>;

/// Sign-in state shared by every ChatGPT request. Tokens are read from disk on first use and cached; the async mutex also serializes refreshes, because refresh tokens rotate and a concurrent second refresh would invalidate the session.
#[derive(Default)]
pub struct Session {
    dir: OnceLock<PathBuf>,
    /// Shared with refresh tasks, which hold the lock until rotated tokens are stored.
    tokens: Arc<tokio::sync::Mutex<TokenSlot>>,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `n` random bytes from the system CSPRNG, base64url-encoded without padding.
fn random_token(n: usize) -> Result<String, String> {
    let mut raw = vec![0u8; n];
    SystemRandom::new()
        .fill(&mut raw)
        .map_err(|_| "no secure random source".to_string())?;
    Ok(URL_SAFE_NO_PAD.encode(raw))
}

/// PKCE S256 challenge of `verifier` (RFC 7636).
fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(ring::digest::digest(
        &ring::digest::SHA256,
        verifier.as_bytes(),
    ))
}

/// A random (version 4) UUID URN, the form `ext_agent_host_id` expects.
fn new_host_id() -> Result<String, String> {
    let mut b = [0u8; 16];
    SystemRandom::new()
        .fill(&mut b)
        .map_err(|_| "no secure random source".to_string())?;
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    Ok(format!(
        "urn:uuid:{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

/// The header, claims, signed part and signature of a JWT, or `None` when it is malformed.
fn jwt_parts(token: &str) -> Option<(Value, Value, &str, Vec<u8>)> {
    let (signed, signature) = token.rsplit_once('.')?;
    let (header, claims) = signed.split_once('.')?;
    let decode = |part: &str| URL_SAFE_NO_PAD.decode(part.trim_end_matches('=')).ok();
    let header: Value = serde_json::from_slice(&decode(header)?).ok()?;
    let claims: Value = serde_json::from_slice(&decode(claims)?).ok()?;
    Some((header, claims, signed, decode(signature)?))
}

/// Verifies an RS256 ID token against the issuer's JWKS and checks issuer, audience, expiry and nonce. Returns the claims.
fn verify_id_token(
    token: &str,
    jwks: &Value,
    client_id: &str,
    nonce: &str,
    now: u64,
) -> Result<Value, String> {
    let (header, claims, signed, signature) = jwt_parts(token).ok_or("malformed ID token")?;
    if header.get("alg").and_then(Value::as_str) != Some("RS256") {
        return Err("unexpected ID token algorithm".to_string());
    }
    let kid = header.get("kid").and_then(Value::as_str);
    let key = jwks
        .get("keys")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|k| k.get("kty").and_then(Value::as_str) == Some("RSA"))
        .find(|k| kid.is_none() || k.get("kid").and_then(Value::as_str) == kid)
        .ok_or("no matching signing key for the ID token")?;
    let component = |name: &str| {
        key.get(name)
            .and_then(Value::as_str)
            .and_then(|v| URL_SAFE_NO_PAD.decode(v.trim_end_matches('=')).ok())
    };
    let n = component("n").ok_or("signing key without modulus")?;
    let e = component("e").ok_or("signing key without exponent")?;
    // ring expects the modulus without leading zero bytes.
    let first = n.iter().position(|&b| b != 0).unwrap_or(n.len());
    let public = RsaPublicKeyComponents {
        n: &n[first..],
        e: e.as_slice(),
    };
    public
        .verify(&RSA_PKCS1_2048_8192_SHA256, signed.as_bytes(), &signature)
        .map_err(|_| "ID token signature is invalid".to_string())?;

    if claims.get("iss").and_then(Value::as_str) != Some(ISSUER) {
        return Err("ID token from an unexpected issuer".to_string());
    }
    let aud_ok = match claims.get("aud") {
        Some(Value::String(aud)) => aud == client_id,
        Some(Value::Array(auds)) => auds.iter().any(|a| a.as_str() == Some(client_id)),
        _ => false,
    };
    if !aud_ok {
        return Err("ID token issued for another client".to_string());
    }
    if claims
        .get("exp")
        .and_then(Value::as_u64)
        .is_none_or(|exp| exp + CLOCK_LEEWAY_SECS < now)
    {
        return Err("ID token has expired".to_string());
    }
    if claims.get("nonce").and_then(Value::as_str) != Some(nonce) {
        return Err("ID token nonce does not match this sign-in".to_string());
    }
    Ok(claims)
}

/// The account e-mail in ID-token claims (top level, or in the profile namespace).
fn email_from_claims(claims: &Value) -> Option<String> {
    claims
        .get("email")
        .or_else(|| claims.pointer("/https:~1~1api.openai.com~1profile/email"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Query parameters of the OAuth redirect that matter here.
#[derive(Debug, Default, PartialEq)]
struct Callback {
    code: Option<String>,
    state: Option<String>,
    client_id: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

/// Parses an HTTP request head into its path and redirect parameters; `None` for anything but a `GET`.
fn parse_request(head: &str) -> Option<(String, Callback)> {
    let line = head.lines().next()?;
    let mut parts = line.split_whitespace();
    if parts.next()? != "GET" {
        return None;
    }
    let url = reqwest::Url::parse(&format!("http://127.0.0.1{}", parts.next()?)).ok()?;
    let mut cb = Callback::default();
    for (key, value) in url.query_pairs() {
        let slot = match key.as_ref() {
            "code" => &mut cb.code,
            "state" => &mut cb.state,
            "client_id" => &mut cb.client_id,
            "error" => &mut cb.error,
            "error_description" => &mut cb.error_description,
            _ => continue,
        };
        *slot = Some(value.into_owned());
    }
    Some((url.path().to_string(), cb))
}

fn page(title: &str, message: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{title}</title></head>\
         <body style=\"font-family:system-ui,sans-serif;max-width:32rem;margin:4rem auto;text-align:center\">\
         <h2>{title}</h2><p>{message}</p></body></html>"
    )
}

async fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    if let Err(e) = stream.write_all(response.as_bytes()).await {
        log::debug!("sign-in callback response failed: {e}");
    }
    let _ = stream.shutdown().await;
}

/// Reads one request head (up to 8 KiB) from a loopback connection.
async fn read_head(stream: &mut TcpStream) -> Option<String> {
    let mut buf = vec![0u8; 8192];
    let mut len = 0;
    loop {
        let n = tokio::time::timeout(CONNECTION_TIMEOUT, stream.read(&mut buf[len..]))
            .await
            .ok()?
            .ok()?;
        if n == 0 {
            break;
        }
        len += n;
        if buf[..len].windows(4).any(|w| w == b"\r\n\r\n") || len == buf.len() {
            break;
        }
    }
    String::from_utf8(buf[..len].to_vec()).ok()
}

/// Waits for the browser to land on the callback with the expected `state`. Unrelated requests (favicon, stale tabs) get a 404 or 400 and the wait continues. The request line is never logged: it carries the authorization code.
async fn wait_for_callback(
    listener: &TcpListener,
    state: &str,
    cancel: &Arc<AtomicBool>,
) -> Result<Callback, String> {
    let deadline = tokio::time::Instant::now() + LOGIN_TIMEOUT;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("cancelled".to_string());
        }
        let accepted = tokio::select! {
            r = listener.accept() => r,
            _ = tokio::time::sleep(Duration::from_millis(200)) => continue,
            _ = tokio::time::sleep_until(deadline) => return Err("sign-in timed out; try again".to_string()),
        };
        let Ok((mut stream, _)) = accepted else {
            continue;
        };
        let Some((path, cb)) = read_head(&mut stream)
            .await
            .as_deref()
            .and_then(parse_request)
        else {
            respond(&mut stream, "400 Bad Request", "").await;
            continue;
        };
        if path != CALLBACK_PATH {
            respond(&mut stream, "404 Not Found", "").await;
            continue;
        }
        if cb.state.as_deref() != Some(state) {
            respond(
                &mut stream,
                "400 Bad Request",
                &page(
                    "Sign-in link expired",
                    "Start the sign-in again from Attributor.",
                ),
            )
            .await;
            continue;
        }
        // An authorization error is returned to the caller, which decides whether the saved registration is still usable.
        if cb.error.is_some() {
            respond(
                &mut stream,
                "200 OK",
                &page(
                    "Sign-in was not completed",
                    "You can close this tab and return to Attributor.",
                ),
            )
            .await;
            return Ok(cb);
        }
        if cb.code.is_none() {
            respond(&mut stream, "400 Bad Request", "").await;
            continue;
        }
        respond(
            &mut stream,
            "200 OK",
            &page(
                "Signed in to ChatGPT",
                "You can close this tab and return to Attributor.",
            ),
        )
        .await;
        return Ok(cb);
    }
}

/// Error code, offending parameter and message of a failed OpenAI response (`{"error":{…}}` or a bare `{"detail":"…"}`).
async fn read_error(resp: reqwest::Response) -> (Option<String>, Option<String>, String) {
    let text = resp.text().await.unwrap_or_default();
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    let error = v.get("error");
    let field = |key: &str| {
        error
            .and_then(|e| e.get(key))
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    let message = field("message")
        .or_else(|| error.and_then(Value::as_str).map(str::to_string))
        .or_else(|| {
            v.get("error_description")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| v.get("detail").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_else(|| http::truncate(text.trim(), 500));
    let code = field("code")
        .or_else(|| field("type"))
        .or_else(|| error.and_then(Value::as_str).map(str::to_string));
    (code, field("param"), message)
}

/// A user-facing explanation for the plan route's documented error codes.
fn plan_error(code: Option<&str>, status: impl std::fmt::Display, message: &str) -> String {
    match code {
        Some("subscription_sharing_usage_limit_exceeded") => {
            "ChatGPT plan usage limit reached; see https://chatgpt.com/settings/usage".to_string()
        }
        Some("subscription_sharing_user_not_eligible") => {
            "this ChatGPT account cannot use its plan in other apps (Plus or Pro is required)"
                .to_string()
        }
        Some("subscription_sharing_usage_unavailable")
        | Some("subscription_sharing_user_unavailable") => {
            "ChatGPT plan usage is temporarily unavailable; try again later".to_string()
        }
        Some("subscription_sharing_invalid_user") => {
            "the ChatGPT sign-in is no longer valid; sign in again".to_string()
        }
        _ => format!("ChatGPT returned {status}: {message}"),
    }
}

/// Refresh errors after which the session cannot recover without a new sign-in.
fn is_terminal_refresh_error(code: Option<&str>) -> bool {
    matches!(
        code,
        Some(
            "invalid_grant"
                | "invalid_refresh_token"
                | "token_expired"
                | "refresh_token_expired"
                | "refresh_token_invalidated"
                | "refresh_token_reused"
        )
    )
}

impl Session {
    /// Sets the directory for the sealed tokens and the registration file (the app's local data directory).
    pub fn set_storage_dir(&self, dir: PathBuf) {
        let _ = self.dir.set(dir);
    }

    fn dir(&self) -> Result<PathBuf, String> {
        self.dir
            .get()
            .cloned()
            .ok_or_else(|| "the app data directory is unavailable".to_string())
    }

    /// Loads the tokens from disk into `slot` once.
    async fn ensure_loaded(&self, slot: &mut TokenSlot) -> Result<(), String> {
        if slot.is_some() {
            return Ok(());
        }
        let dir = self.dir()?;
        let raw = tokio::task::spawn_blocking(move || secrets::read_sealed(&dir, VAULT_NAME))
            .await
            .map_err(|e| e.to_string())??;
        *slot = Some(raw.and_then(|bytes| serde_json::from_slice(&bytes).ok()));
        Ok(())
    }

    /// The signed-in account, or `None` when signed out.
    ///
    /// # Errors
    ///
    /// Returns an error when the token store cannot be read.
    pub async fn account(&self) -> Result<Option<ChatGptAccount>, String> {
        let mut slot = self.tokens.lock().await;
        self.ensure_loaded(&mut slot).await?;
        Ok(slot
            .as_ref()
            .and_then(Option::as_ref)
            .map(|t| ChatGptAccount {
                email: t.email.clone(),
            }))
    }

    /// A valid access token, refreshed first when it expires within five minutes (or when `force` is set, after a 401).
    ///
    /// The refresh runs in its own task that keeps the lock until the rotated tokens are cached and stored, so canceling the request that triggered it cannot lose the only valid refresh token.
    async fn access_token(&self, force: bool) -> Result<String, String> {
        let dir = self.dir()?;
        let mut slot = self.tokens.clone().lock_owned().await;
        self.ensure_loaded(&mut slot).await?;
        let tokens = slot
            .as_ref()
            .and_then(Option::as_ref)
            .cloned()
            .ok_or("not signed in to ChatGPT (Settings → AI attribution)")?;
        if !force && tokens.expires_at > now_secs().saturating_add(REFRESH_MARGIN_SECS) {
            return Ok(tokens.access_token);
        }
        tokio::spawn(async move {
            match refresh(&tokens).await {
                Ok(fresh) => {
                    let token = fresh.access_token.clone();
                    *slot = Some(Some(fresh.clone()));
                    // The cached copy keeps this session working even if storing fails; only a restart would then need a new sign-in.
                    if let Err(e) = persist_tokens(&dir, &fresh).await {
                        log::warn!("could not store the refreshed ChatGPT tokens: {e}");
                    }
                    Ok(token)
                }
                Err(RefreshError::Transient(message)) => Err(message),
                Err(terminal) => {
                    log::warn!("ChatGPT refresh rejected; signing out");
                    *slot = Some(None);
                    if let Err(e) = delete_tokens(&dir).await {
                        log::warn!("could not delete the rejected ChatGPT tokens: {e}");
                    }
                    if matches!(terminal, RefreshError::UnknownClient) {
                        forget_client(&dir).await;
                    }
                    Err("the ChatGPT sign-in has expired; sign in again (Settings → AI attribution)".to_string())
                }
            }
        })
        .await
        .map_err(|e| e.to_string())?
    }

    /// Signs in with ChatGPT: opens the browser on OpenAI's consent page, waits for the loopback redirect, exchanges the code and verifies the ID token. Cancelable through `cancel`.
    ///
    /// The client id issued on an earlier sign-in is reused; `new_account` registers the app again instead, for signing in with a different ChatGPT account or workspace.
    /// `on_link` receives the sign-in page's address before the browser opens, so the app can offer it for another browser; a browser that fails to open is therefore only logged, and the sign-in keeps waiting.
    ///
    /// # Errors
    ///
    /// Returns `"cancelled"` when canceled, or an error when the user declines plan usage, the sign-in times out, or any OAuth step fails.
    pub async fn login(
        &self,
        app: &tauri::AppHandle,
        cancel: &Arc<AtomicBool>,
        new_account: bool,
        on_link: impl FnOnce(&str) + Send,
    ) -> Result<ChatGptAccount, String> {
        use tauri_plugin_opener::OpenerExt;

        let dir = self.dir()?;
        let mut registration = load_registration(&dir).await?;
        if new_account {
            registration.client_id = None;
            registration.subject = None;
        }
        let verifier = random_token(64)?;
        let state = random_token(32)?;
        let nonce = random_token(32)?;

        // Listen before opening the browser; an OS-chosen port avoids clashing with other apps' sign-in servers.
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|e| format!("could not open the sign-in callback port: {e}"))?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let redirect_uri = format!("http://127.0.0.1:{port}{CALLBACK_PATH}");

        let saved_client = registration.client_id.clone();
        let challenge = pkce_challenge(&verifier);
        let url = authorize_url(
            saved_client.as_deref(),
            &registration,
            &redirect_uri,
            &state,
            &nonce,
            &challenge,
        )?;
        log::info!("chatgpt login: waiting for the browser redirect on port {port}");
        on_link(url.as_str());
        if let Err(e) = app.opener().open_url(url.as_str(), None::<&str>) {
            log::warn!("chatgpt login: could not open the browser ({e}); the link can be copied in the app");
        }

        let callback = wait_for_callback(&listener, &state, cancel).await?;
        drop(listener);
        if let Some(error) = &callback.error {
            if saved_client.is_some()
                && matches!(error.as_str(), "invalid_client" | "unauthorized_client")
            {
                // The saved registration is no longer accepted; the next sign-in registers the app again.
                forget_client(&dir).await;
            }
            let detail = callback.error_description.as_deref().unwrap_or_default();
            return Err(format!("sign-in was declined ({error}) {detail}")
                .trim()
                .to_string());
        }
        let code = callback
            .code
            .ok_or("the sign-in redirect carried no authorization code")?;

        // The callback names the issued client on a registering sign-in; later sign-ins must keep the same registration.
        let issued = match (saved_client, callback.client_id) {
            (Some(saved), Some(returned)) if saved != returned => {
                return Err("ChatGPT returned a different app registration; use \"Use another account\" to sign in with a different account".to_string())
            }
            (Some(saved), _) => saved,
            (None, Some(returned)) => returned,
            (None, None) => {
                return Err("ChatGPT did not register the app (no client id in the redirect)".to_string())
            }
        };
        if registration.client_id.as_deref() != Some(issued.as_str()) {
            registration.client_id = Some(issued.clone());
            save_registration(&dir, &registration).await?;
        }

        let resp = http::client()
            .post(TOKEN_URL)
            .form(&[
                ("grant_type", "authorization_code"),
                ("client_id", issued.as_str()),
                ("code", code.as_str()),
                ("code_verifier", verifier.as_str()),
                ("redirect_uri", redirect_uri.as_str()),
                ("resource", RESOURCE),
            ])
            .timeout(http::QUICK_TIMEOUT)
            .send()
            .await
            .map_err(|e| format!("ChatGPT sign-in failed: {e}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let (code, _, message) = read_error(resp).await;
            if code.as_deref() == Some("invalid_client") {
                forget_client(&dir).await;
                return Err(
                    "ChatGPT no longer recognizes this app registration; sign in again".to_string(),
                );
            }
            return Err(format!("ChatGPT sign-in failed ({status}): {message}"));
        }
        let tokens: TokenResponse = resp.json().await.map_err(|e| e.to_string())?;
        if !tokens
            .scope
            .as_deref()
            .unwrap_or_default()
            .split_whitespace()
            .any(|s| s == PLAN_SCOPE)
        {
            // Without this flag, a returning sign-in would skip the consent screen and fail the same way every time.
            registration.needs_consent = true;
            save_registration(&dir, &registration).await?;
            return Err(
                "ChatGPT plan usage was not shared with Attributor; sign in again and allow it"
                    .to_string(),
            );
        }
        let id_token = tokens
            .id_token
            .as_deref()
            .ok_or("ChatGPT returned no ID token")?;
        let jwks: Value = http::client()
            .get(JWKS_URL)
            .timeout(http::QUICK_TIMEOUT)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        let claims = verify_id_token(id_token, &jwks, &issued, &nonce, now_secs())?;

        let subject = claims
            .get("sub")
            .and_then(Value::as_str)
            .map(str::to_string);
        if registration.needs_consent || registration.subject != subject {
            registration.needs_consent = false;
            registration.subject = subject;
            save_registration(&dir, &registration).await?;
        }
        let stored = Tokens {
            client_id: issued,
            access_token: tokens.access_token,
            refresh_token: tokens
                .refresh_token
                .ok_or("ChatGPT returned no refresh token")?,
            expires_at: now_secs().saturating_add(tokens.expires_in.unwrap_or(3600)),
            email: email_from_claims(&claims),
        };
        persist_tokens(&dir, &stored).await?;
        let account = ChatGptAccount {
            email: stored.email.clone(),
        };
        *self.tokens.lock().await = Some(Some(stored));
        log::info!("chatgpt login: signed in");
        Ok(account)
    }

    /// Signs out: revokes the refresh token (best effort) and deletes the stored tokens. The app registration is kept for the next sign-in.
    ///
    /// # Errors
    ///
    /// Returns an error when the stored tokens cannot be deleted; the session then stays signed in.
    pub async fn logout(&self) -> Result<(), String> {
        let dir = self.dir()?;
        let mut slot = self.tokens.lock().await;
        self.ensure_loaded(&mut slot).await?;
        if let Some(tokens) = slot.as_ref().and_then(Option::as_ref) {
            let revoke = http::client()
                .post(REVOKE_URL)
                .form(&[
                    ("token", tokens.refresh_token.as_str()),
                    ("token_type_hint", "refresh_token"),
                    ("client_id", tokens.client_id.as_str()),
                ])
                .timeout(Duration::from_secs(10))
                .send()
                .await;
            match revoke {
                Ok(resp) if resp.status().is_success() => {}
                Ok(resp) => log::warn!("ChatGPT token revocation returned {}", resp.status()),
                Err(e) => log::warn!("ChatGPT token revocation failed: {e}"),
            }
        }
        delete_tokens(&dir).await?;
        *slot = Some(None);
        Ok(())
    }

    /// Checks that the sign-in works by reading the model catalog.
    ///
    /// # Errors
    ///
    /// Returns why ChatGPT cannot be used (signed out, expired, not eligible, unreachable).
    pub async fn check(&self) -> Result<(), String> {
        self.fetch_models().await.map(|_| ())
    }

    /// Vision models available to the plan; falls back to a built-in list when the catalog is empty or unreadable.
    ///
    /// # Errors
    ///
    /// Returns an error only when signed out or the sign-in is no longer valid.
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>, String> {
        match self.fetch_models().await {
            Ok(models) if !models.is_empty() => Ok(models),
            Ok(_) => Ok(fallback_models()),
            Err(e) if e.contains("sign in") || e.contains("signed in") => Err(e),
            Err(e) => {
                log::warn!("ChatGPT model catalog unavailable ({e}); using the built-in list");
                Ok(fallback_models())
            }
        }
    }

    async fn fetch_models(&self) -> Result<Vec<ModelInfo>, String> {
        let mut force = false;
        loop {
            let token = self.access_token(force).await?;
            let resp = http::client()
                .get(http::url(API_BASE, "models"))
                .bearer_auth(token)
                .timeout(http::QUICK_TIMEOUT)
                .send()
                .await
                .map_err(|e| format!("ChatGPT not reachable: {e}"))?;
            let status = resp.status();
            if status == reqwest::StatusCode::UNAUTHORIZED && !force {
                force = true;
                continue;
            }
            if !status.is_success() {
                let (code, _, message) = read_error(resp).await;
                return Err(plan_error(code.as_deref(), status, &message));
            }
            let body: Value = resp.json().await.map_err(|e| e.to_string())?;
            return Ok(catalog_models(&body));
        }
    }
}

/// Error of a refresh attempt.
enum RefreshError {
    /// The refresh token is no longer usable; a new sign-in is needed.
    Terminal,
    /// The issued client id itself was rejected; the next sign-in must register the app again.
    UnknownClient,
    /// A temporary failure; the tokens stay valid.
    Transient(String),
}

/// The authorization URL. A registering sign-in (`client_id` is `None`) uses the reserved client and names the app; a returning one uses the issued client and asks for consent again when the user declined plan sharing last time.
fn authorize_url(
    client_id: Option<&str>,
    registration: &Registration,
    redirect_uri: &str,
    state: &str,
    nonce: &str,
    challenge: &str,
) -> Result<reqwest::Url, String> {
    let mut params: Vec<(&str, &str)> =
        vec![("client_id", client_id.unwrap_or(REGISTRATION_CLIENT_ID))];
    match client_id {
        None => params.push(("agent_name_hint", AGENT_NAME)),
        Some(_) if registration.needs_consent => params.push(("prompt", "consent")),
        Some(_) => {}
    }
    params.extend([
        ("ext_agent_host_id", registration.host_id.as_str()),
        ("response_type", "code"),
        ("redirect_uri", redirect_uri),
        ("scope", SCOPE),
        ("resource", RESOURCE),
        ("state", state),
        ("nonce", nonce),
        ("code_challenge_method", "S256"),
        ("code_challenge", challenge),
    ]);
    reqwest::Url::parse_with_params(AUTHORIZE_URL, &params).map_err(|e| e.to_string())
}

async fn persist_tokens(dir: &Path, tokens: &Tokens) -> Result<(), String> {
    let data = serde_json::to_vec(tokens).map_err(|e| e.to_string())?;
    let dir = dir.to_path_buf();
    tokio::task::spawn_blocking(move || secrets::write_sealed(&dir, VAULT_NAME, &data))
        .await
        .map_err(|e| e.to_string())?
}

async fn delete_tokens(dir: &Path) -> Result<(), String> {
    let dir = dir.to_path_buf();
    tokio::task::spawn_blocking(move || secrets::delete_sealed(&dir, VAULT_NAME))
        .await
        .map_err(|e| e.to_string())?
}

/// Drops the issued client id (keeping the host id), so the next sign-in registers the app again.
async fn forget_client(dir: &Path) {
    let reset = async {
        let mut registration = load_registration(dir).await?;
        registration.client_id = None;
        registration.subject = None;
        save_registration(dir, &registration).await
    };
    if let Err(e) = reset.await {
        log::warn!("could not reset the ChatGPT app registration: {e}");
    }
}

/// Exchanges the refresh token for new tokens (the refresh token rotates on every use).
async fn refresh(tokens: &Tokens) -> Result<Tokens, RefreshError> {
    let resp = http::client()
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", tokens.client_id.as_str()),
            ("refresh_token", tokens.refresh_token.as_str()),
            ("resource", RESOURCE),
        ])
        .timeout(http::QUICK_TIMEOUT)
        .send()
        .await
        .map_err(|e| RefreshError::Transient(format!("ChatGPT not reachable: {e}")))?;
    let status = resp.status();
    if !status.is_success() {
        let (code, _, message) = read_error(resp).await;
        log::warn!("ChatGPT token refresh failed ({status}): {message}");
        return Err(if code.as_deref() == Some("invalid_client") {
            RefreshError::UnknownClient
        } else if status == reqwest::StatusCode::UNAUTHORIZED
            || is_terminal_refresh_error(code.as_deref())
        {
            RefreshError::Terminal
        } else {
            RefreshError::Transient(format!(
                "ChatGPT token refresh failed ({status}): {message}"
            ))
        });
    }
    let fresh: TokenResponse = resp
        .json()
        .await
        .map_err(|e| RefreshError::Transient(e.to_string()))?;
    let email = fresh
        .id_token
        .as_deref()
        .and_then(jwt_parts)
        .and_then(|(_, claims, _, _)| email_from_claims(&claims))
        .or_else(|| tokens.email.clone());
    Ok(Tokens {
        client_id: tokens.client_id.clone(),
        access_token: fresh.access_token,
        refresh_token: fresh
            .refresh_token
            .unwrap_or_else(|| tokens.refresh_token.clone()),
        expires_at: now_secs().saturating_add(fresh.expires_in.unwrap_or(3600)),
        email,
    })
}

async fn load_registration(dir: &Path) -> Result<Registration, String> {
    let path = dir.join(REGISTRATION_FILE);
    let existing = tokio::fs::read(&path)
        .await
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Registration>(&bytes).ok())
        .filter(|r| !r.host_id.is_empty());
    match existing {
        Some(registration) => Ok(registration),
        None => {
            let registration = Registration {
                host_id: new_host_id()?,
                ..Default::default()
            };
            save_registration(dir, &registration).await?;
            Ok(registration)
        }
    }
}

async fn save_registration(dir: &Path, registration: &Registration) -> Result<(), String> {
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|e| e.to_string())?;
    let data = serde_json::to_vec_pretty(registration).map_err(|e| e.to_string())?;
    tokio::fs::write(dir.join(REGISTRATION_FILE), data)
        .await
        .map_err(|e| e.to_string())
}

fn fallback_models() -> Vec<ModelInfo> {
    FALLBACK_MODELS
        .iter()
        .map(|id| ModelInfo {
            name: (*id).to_string(),
            size: 0,
        })
        .collect()
}

/// Listed vision models from the plan catalog (`{"models":[{"slug", "visibility", "supported_in_api", "input_modalities"}]}`), or from a standard `{"data":[{"id"}]}` list.
fn catalog_models(body: &Value) -> Vec<ModelInfo> {
    if let Some(models) = body.get("models").and_then(Value::as_array) {
        return models
            .iter()
            .filter(|m| {
                m.get("visibility")
                    .and_then(Value::as_str)
                    .is_none_or(|v| v == "list")
            })
            .filter(|m| m.get("supported_in_api").and_then(Value::as_bool) != Some(false))
            .filter(|m| {
                m.get("input_modalities")
                    .and_then(Value::as_array)
                    .is_none_or(|ms| ms.iter().any(|x| x.as_str() == Some("image")))
            })
            .filter_map(|m| {
                m.get("slug")
                    .or_else(|| m.get("id"))
                    .and_then(Value::as_str)
            })
            .map(|id| ModelInfo {
                name: id.to_string(),
                size: 0,
            })
            .collect();
    }
    // The standard list carries no capabilities, so it is filtered by id like the OpenAI API provider's list.
    body.get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|m| m.get("id").and_then(Value::as_str))
        .filter(|id| openai::is_vision_chat_model(id))
        .map(|id| ModelInfo {
            name: id.to_string(),
            size: 0,
        })
        .collect()
}

/// Folds the Responses SSE stream into the answer text. Only `response.completed` counts as success; usage-limit errors can arrive as `response.failed` after an HTTP 200.
async fn read_stream(resp: reqwest::Response) -> Result<String, String> {
    let mut stream = resp.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();
    let mut acc = StreamAccumulator::default();
    while let Some(chunk) = stream.next().await {
        buf.extend_from_slice(&chunk.map_err(|e| format!("ChatGPT stream interrupted: {e}"))?);
        while let Some(pos) = find_event_end(&buf) {
            let event: Vec<u8> = buf.drain(..pos).collect();
            if let Some(done) = acc.feed(&String::from_utf8_lossy(&event))? {
                return Ok(done);
            }
        }
    }
    if !buf.is_empty() {
        if let Some(done) = acc.feed(&String::from_utf8_lossy(&buf))? {
            return Ok(done);
        }
    }
    Err("ChatGPT stream ended before the answer was complete".to_string())
}

/// End (exclusive, including the blank line) of the first complete SSE event in `buf`.
fn find_event_end(buf: &[u8]) -> Option<usize> {
    let lf = buf.windows(2).position(|w| w == b"\n\n").map(|p| p + 2);
    let crlf = buf.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4);
    match (lf, crlf) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

/// Collects answer text from Responses stream events.
#[derive(Default)]
struct StreamAccumulator {
    deltas: String,
    items: Vec<Value>,
}

impl StreamAccumulator {
    /// Processes one SSE event; returns the final text once the response completes.
    fn feed(&mut self, event: &str) -> Result<Option<String>, String> {
        let data: String = event
            .lines()
            .filter_map(|l| l.strip_prefix("data:"))
            .map(str::trim_start)
            .collect::<Vec<_>>()
            .join("\n");
        if data.is_empty() || data == "[DONE]" {
            return Ok(None);
        }
        let Ok(v) = serde_json::from_str::<Value>(&data) else {
            return Ok(None);
        };
        match v.get("type").and_then(Value::as_str) {
            Some("response.output_text.delta") => self
                .deltas
                .push_str(v.get("delta").and_then(Value::as_str).unwrap_or_default()),
            Some("response.output_item.done") => {
                if let Some(item) = v
                    .get("item")
                    .filter(|i| i.get("type").and_then(Value::as_str) == Some("message"))
                {
                    self.items.push(item.clone());
                }
            }
            Some("response.completed") => {
                let synthetic = json!({"status": "completed", "output": self.items});
                return match openai::answer_text(&synthetic, "ChatGPT") {
                    Ok(text) => Ok(Some(text)),
                    Err(_) if !self.deltas.trim().is_empty() => {
                        Ok(Some(std::mem::take(&mut self.deltas)))
                    }
                    Err(e) => Err(e),
                };
            }
            Some("response.incomplete") => {
                let reason = v
                    .pointer("/response/incomplete_details/reason")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                return Err(format!(
                    "ChatGPT stopped before finishing the answer ({reason})"
                ));
            }
            Some("response.failed") | Some("error") => {
                let error = v
                    .pointer("/response/error")
                    .or_else(|| v.get("error"))
                    .unwrap_or(&v);
                let code = error.get("code").and_then(Value::as_str);
                let message = error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown error");
                return Err(plan_error(code, "an error", message));
            }
            _ => {}
        }
        Ok(None)
    }
}

/// One vision inference on the user's ChatGPT plan. Retries once with refreshed tokens after a 401, and once without `text.format` if the plan route rejects structured output (the schema is still appended to the prompt).
///
/// # Errors
///
/// Returns an error when signed out, on transport failures, plan errors (usage limit, eligibility) and incomplete answers.
pub async fn generate(
    cfg: &AttributionConfig,
    session: &Session,
    image: &ImagePayload,
) -> Result<String, String> {
    let mut body = openai::request_body(cfg, image);
    body.insert("stream".into(), json!(true));
    log::info!(
        "chatgpt generate → model={} options={} think={:?} promptChars={}",
        cfg.model,
        cfg.options,
        cfg.think,
        cfg.prompt.chars().count()
    );
    let mut refreshed = false;
    loop {
        let token = session.access_token(refreshed).await?;
        let resp = http::client()
            .post(http::url(API_BASE, "responses"))
            .bearer_auth(token)
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .json(&body)
            .timeout(http::GENERATE_TIMEOUT)
            .send()
            .await
            .map_err(|e| format!("ChatGPT not reachable: {e}"))?;
        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED && !refreshed {
            refreshed = true;
            continue;
        }
        if !status.is_success() {
            let (code, param, message) = read_error(resp).await;
            let format_rejected = code.as_deref()
                == Some("subscription_sharing_unsupported_capability")
                && param.as_deref().is_some_and(|p| p.starts_with("text"));
            if format_rejected && body.remove("text").is_some() {
                log::warn!("ChatGPT plan route rejected structured output; retrying with the schema in the prompt only");
                continue;
            }
            return Err(plan_error(code.as_deref(), status, &message));
        }
        let text = read_stream(resp).await?;
        log::info!(
            "chatgpt generate ← model={} responseChars={}",
            cfg.model,
            text.chars().count()
        );
        log::debug!("chatgpt generate raw response: {text}");
        return Ok(text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::signature::{RsaKeyPair, RSA_PKCS1_SHA256};

    const TEST_KEY: &[u8] = include_bytes!("testdata/test_rsa2048.pk8");

    /// Signs `claims` as an RS256 JWT with the test key and returns it with the matching JWKS.
    fn signed_token(claims: &Value) -> (String, Value) {
        let key = RsaKeyPair::from_pkcs8(TEST_KEY).unwrap();
        let header = URL_SAFE_NO_PAD.encode(json!({"alg": "RS256", "kid": "k1"}).to_string());
        let payload = URL_SAFE_NO_PAD.encode(claims.to_string());
        let signed = format!("{header}.{payload}");
        let mut sig = vec![0u8; key.public().modulus_len()];
        key.sign(
            &RSA_PKCS1_SHA256,
            &SystemRandom::new(),
            signed.as_bytes(),
            &mut sig,
        )
        .unwrap();
        let public: RsaPublicKeyComponents<Vec<u8>> = key.public().into();
        let jwks = json!({"keys": [{"kty": "RSA", "kid": "k1", "n": URL_SAFE_NO_PAD.encode(&public.n), "e": URL_SAFE_NO_PAD.encode(&public.e)}]});
        (format!("{signed}.{}", URL_SAFE_NO_PAD.encode(sig)), jwks)
    }

    fn claims(now: u64) -> Value {
        json!({"iss": ISSUER, "aud": "oaiapp_1", "exp": now + 600, "nonce": "n1", "email": "a@b.c", "sub": "user"})
    }

    #[test]
    fn id_token_verification_accepts_valid_tokens() {
        let now = 1_800_000_000;
        let (token, jwks) = signed_token(&claims(now));
        let verified = verify_id_token(&token, &jwks, "oaiapp_1", "n1", now).unwrap();
        assert_eq!(email_from_claims(&verified).as_deref(), Some("a@b.c"));
    }

    #[test]
    fn id_token_verification_rejects_wrong_claims_and_tampering() {
        let now = 1_800_000_000;
        let (token, jwks) = signed_token(&claims(now));
        assert!(
            verify_id_token(&token, &jwks, "oaiapp_2", "n1", now).is_err(),
            "audience"
        );
        assert!(
            verify_id_token(&token, &jwks, "oaiapp_1", "other", now).is_err(),
            "nonce"
        );
        assert!(
            verify_id_token(&token, &jwks, "oaiapp_1", "n1", now + 3600).is_err(),
            "expiry"
        );
        let mut forged = claims(now);
        forged["email"] = json!("evil@x.y");
        let (other, _) = signed_token(&forged);
        let (head_sig, _) = token.split_once('.').unwrap();
        let payload = other.split('.').nth(1).unwrap();
        let tampered = format!("{head_sig}.{payload}.{}", token.rsplit('.').next().unwrap());
        assert!(
            verify_id_token(&tampered, &jwks, "oaiapp_1", "n1", now).is_err(),
            "signature"
        );
    }

    #[test]
    fn pkce_challenge_matches_rfc7636_example() {
        // RFC 7636, Appendix B.
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn host_id_is_a_v4_uuid_urn() {
        let id = new_host_id().unwrap();
        let uuid = id.strip_prefix("urn:uuid:").unwrap();
        assert_eq!(uuid.len(), 36);
        assert_eq!(uuid.as_bytes()[14], b'4');
        assert!(matches!(uuid.as_bytes()[19], b'8' | b'9' | b'a' | b'b'));
    }

    #[test]
    fn callback_request_is_parsed() {
        let head = "GET /auth/callback?code=abc&state=s%2B1&client_id=oaiapp_9&scope=x HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        let (path, cb) = parse_request(head).unwrap();
        assert_eq!(path, "/auth/callback");
        assert_eq!(cb.code.as_deref(), Some("abc"));
        assert_eq!(cb.state.as_deref(), Some("s+1"));
        assert_eq!(cb.client_id.as_deref(), Some("oaiapp_9"));
        assert!(parse_request("POST /auth/callback HTTP/1.1\r\n\r\n").is_none());
    }

    #[test]
    fn stream_accumulator_reads_items_and_fails_on_errors() {
        let mut acc = StreamAccumulator::default();
        assert!(acc.feed("event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"{\\\"a\\\"\"}").unwrap().is_none());
        let item = json!({"type": "response.output_item.done", "item": {"type": "message", "content": [{"type": "output_text", "text": "{\"a\":1}"}]}});
        assert!(acc.feed(&format!("data: {item}")).unwrap().is_none());
        assert_eq!(
            acc.feed("data: {\"type\":\"response.completed\",\"response\":{}}")
                .unwrap()
                .as_deref(),
            Some("{\"a\":1}")
        );

        let mut failed = StreamAccumulator::default();
        let event = json!({"type": "response.failed", "response": {"error": {"code": "subscription_sharing_usage_limit_exceeded", "message": "limit"}}});
        assert!(failed
            .feed(&format!("data: {event}"))
            .unwrap_err()
            .contains("usage limit"));
    }

    #[test]
    fn stream_accumulator_falls_back_to_deltas() {
        let mut acc = StreamAccumulator::default();
        acc.feed("data: {\"type\":\"response.output_text.delta\",\"delta\":\"{}\"}")
            .unwrap();
        assert_eq!(
            acc.feed("data: {\"type\":\"response.completed\"}")
                .unwrap()
                .as_deref(),
            Some("{}")
        );
    }

    #[test]
    fn sse_event_boundaries_handle_lf_and_crlf() {
        assert_eq!(find_event_end(b"data: x\n\nrest"), Some(9));
        assert_eq!(find_event_end(b"data: x\r\n\r\nrest"), Some(11));
        assert_eq!(find_event_end(b"data: x\n"), None);
    }

    #[test]
    fn catalog_keeps_listed_vision_models() {
        let body = json!({"models": [
            {"slug": "gpt-6.1-sol", "visibility": "list", "supported_in_api": true, "input_modalities": ["text", "image"]},
            {"slug": "codex-auto-review", "visibility": "hide"},
            {"slug": "text-only", "visibility": "list", "input_modalities": ["text"]}
        ]});
        let names: Vec<String> = catalog_models(&body).into_iter().map(|m| m.name).collect();
        assert_eq!(names, ["gpt-6.1-sol"]);
    }

    #[test]
    fn plan_errors_are_explained() {
        assert!(
            plan_error(Some("subscription_sharing_user_not_eligible"), 403, "x")
                .contains("Plus or Pro")
        );
        assert!(plan_error(None, 500, "boom").contains("boom"));
    }
}

#[cfg(test)]
mod authorize_tests {
    use super::*;

    fn params(url: &reqwest::Url) -> std::collections::HashMap<String, String> {
        url.query_pairs().into_owned().collect()
    }

    #[test]
    fn registering_sign_in_uses_the_reserved_client() {
        let reg = Registration {
            host_id: "urn:uuid:x".into(),
            ..Default::default()
        };
        let p = params(
            &authorize_url(
                None,
                &reg,
                "http://127.0.0.1:5/auth/callback",
                "s",
                "n",
                "c",
            )
            .unwrap(),
        );
        assert_eq!(p["client_id"], REGISTRATION_CLIENT_ID);
        assert_eq!(p["agent_name_hint"], AGENT_NAME);
        assert_eq!(p["resource"], RESOURCE);
        assert_eq!(p["scope"], SCOPE);
        assert_eq!(p["code_challenge_method"], "S256");
        assert!(!p.contains_key("prompt"));
    }

    #[test]
    fn returning_sign_in_asks_for_consent_only_after_a_decline() {
        let mut reg = Registration {
            host_id: "urn:uuid:x".into(),
            client_id: Some("oaiapp_1".into()),
            ..Default::default()
        };
        let p = params(
            &authorize_url(
                Some("oaiapp_1"),
                &reg,
                "http://127.0.0.1:5/auth/callback",
                "s",
                "n",
                "c",
            )
            .unwrap(),
        );
        assert_eq!(p["client_id"], "oaiapp_1");
        assert!(!p.contains_key("agent_name_hint") && !p.contains_key("prompt"));
        reg.needs_consent = true;
        let p = params(
            &authorize_url(
                Some("oaiapp_1"),
                &reg,
                "http://127.0.0.1:5/auth/callback",
                "s",
                "n",
                "c",
            )
            .unwrap(),
        );
        assert_eq!(p["prompt"], "consent");
    }
}
