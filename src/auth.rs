use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
use subtle::ConstantTimeEq;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::state::AppState;

pub const SESSION_COOKIE: &str = "fu_session";
/// Sessions live for seven days unless the server restarts. Tokens are kept
/// in memory only, so a restart always invalidates every session.
pub const SESSION_TTL_SECS: u64 = 7 * 24 * 60 * 60;
const PBKDF2_ITERATIONS: u32 = 210_000;
const HASH_LEN: usize = 32;
const PASSWORD_MIN_LEN: usize = 8;
const PASSWORD_MAX_LEN: usize = 128;
const USERNAME_MAX_LEN: usize = 64;

// ---------------------------------------------------------------------------
// Session store
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Session {
    pub username: String,
    pub expires_at: Instant,
}

#[derive(Default)]
pub struct SessionStore {
    sessions: RwLock<HashMap<String, Session>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn insert(&self, token: String, username: String) {
        let mut sessions = self.sessions.write().await;
        sessions.insert(
            token,
            Session {
                username,
                expires_at: Instant::now() + Duration::from_secs(SESSION_TTL_SECS),
            },
        );
    }

    /// Returns the username when the token is valid. Expired entries are
    /// dropped on access.
    pub async fn validate(&self, token: &str) -> Option<String> {
        let mut sessions = self.sessions.write().await;
        match sessions.get(token) {
            Some(session) if session.expires_at > Instant::now() => Some(session.username.clone()),
            Some(_) => {
                sessions.remove(token);
                None
            }
            None => None,
        }
    }

    pub async fn remove(&self, token: &str) {
        self.sessions.write().await.remove(token);
    }

    pub async fn clear(&self) {
        self.sessions.write().await.clear();
    }
}

// ---------------------------------------------------------------------------
// Passwords
// ---------------------------------------------------------------------------

pub fn new_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

pub fn new_salt() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

pub fn hash_password(password: &str, salt: &str, iterations: u32) -> String {
    let mut output = [0u8; HASH_LEN];
    pbkdf2_hmac::<Sha256>(
        password.as_bytes(),
        salt.as_bytes(),
        iterations.max(1),
        &mut output,
    );
    STANDARD.encode(output)
}

pub fn verify_password(password: &str, salt: &str, iterations: u32, expected: &str) -> bool {
    let Ok(expected) = STANDARD.decode(expected) else {
        return false;
    };
    let mut output = [0u8; HASH_LEN];
    pbkdf2_hmac::<Sha256>(
        password.as_bytes(),
        salt.as_bytes(),
        iterations.max(1),
        &mut output,
    );
    constant_time_eq(&output, &expected)
}

pub fn validate_credentials(username: &str, password: &str) -> Result<(), &'static str> {
    let username = username.trim();
    if username.is_empty() {
        return Err("用户名不能为空");
    }
    if username.len() > USERNAME_MAX_LEN {
        return Err("用户名过长");
    }
    if username.contains(':') || username.contains(char::is_whitespace) {
        return Err("用户名不能包含空格或冒号");
    }
    if password.len() < PASSWORD_MIN_LEN {
        return Err("密码至少需要 8 个字符");
    }
    if password.len() > PASSWORD_MAX_LEN {
        return Err("密码过长");
    }
    Ok(())
}

pub const PASSWORD_ITERATIONS: u32 = PBKDF2_ITERATIONS;

// ---------------------------------------------------------------------------
// Cookies & headers
// ---------------------------------------------------------------------------

pub fn session_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .find_map(|part| {
            let (name, value) = part.trim().split_once('=')?;
            (name == SESSION_COOKIE).then(|| value.trim().to_string())
        })
        .filter(|value| !value.is_empty())
}

pub fn request_is_secure(headers: &HeaderMap) -> bool {
    headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value
                .split(',')
                .next()
                .map(|item| item.trim().eq_ignore_ascii_case("https"))
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

pub fn session_cookie(token: &str, secure: bool) -> String {
    format!(
        "{name}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max}{secure}",
        name = SESSION_COOKIE,
        token = token,
        max = SESSION_TTL_SECS,
        secure = if secure { "; Secure" } else { "" },
    )
}

pub fn clear_session_cookie() -> String {
    format!(
        "{name}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
        name = SESSION_COOKIE
    )
}

pub fn set_cookie_header(value: String) -> (header::HeaderName, HeaderValue) {
    (
        header::SET_COOKIE,
        HeaderValue::from_str(&value)
            .unwrap_or_else(|_| HeaderValue::from_static("fu_session=; Path=/; Max-Age=0")),
    )
}

// ---------------------------------------------------------------------------
// Middleware
// ---------------------------------------------------------------------------

/// HTTP Basic authentication for the whole application (optional).
///
/// When `APP_BASIC_AUTH_USERNAME`/`APP_BASIC_AUTH_PASSWORD` are configured this
/// runs in front of the session login and counts as a valid authentication, so
/// existing Basic-auth deployments keep working without a second login.
///
/// `/healthz` stays unauthenticated so container/systemd health checks work.
/// Browsers cache Basic credentials and reuse them for same-origin fetch and
/// EventSource/SSE requests after the initial challenge.
pub async fn basic_auth(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let Some(config) = state.config.basic_auth.as_ref() else {
        return next.run(request).await;
    };

    if request.uri().path() == "/healthz" {
        return next.run(request).await;
    }

    if basic_credentials_ok(config, request.headers()) {
        return next.run(request).await;
    }

    let mut response = (StatusCode::UNAUTHORIZED, "Authentication required\n").into_response();
    response.headers_mut().insert(
        header::WWW_AUTHENTICATE,
        HeaderValue::from_static("Basic realm=\"FileUploader\", charset=\"UTF-8\""),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// Protects the JSON/SSE API with a session cookie. Static assets and the SPA
/// stay public so the login page itself can be served; the UI decides what to
/// render from `/api/auth/status`.
pub async fn require_session(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    if path == "/healthz"
        || !path.starts_with("/api")
        || matches!(
            path,
            "/api/auth/status" | "/api/auth/login" | "/api/auth/setup" | "/api/auth/logout"
        )
    {
        return next.run(request).await;
    }

    // An already authenticated Basic-auth request counts as logged in.
    if state
        .config
        .basic_auth
        .as_ref()
        .is_some_and(|config| basic_credentials_ok(config, request.headers()))
    {
        return next.run(request).await;
    }

    if let Some(token) = session_token(request.headers()) {
        if state.sessions.validate(&token).await.is_some() {
            return next.run(request).await;
        }
    }

    unauthorized_response()
}

pub fn basic_credentials_ok(config: &crate::config::BasicAuthConfig, headers: &HeaderMap) -> bool {
    let credentials = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_basic_credentials);

    match credentials {
        Some((username, password)) => {
            let username_ok = constant_time_eq(username.as_bytes(), config.username.as_bytes());
            let password_ok = constant_time_eq(password.as_bytes(), config.password.as_bytes());
            username_ok & password_ok
        }
        None => false,
    }
}

pub fn basic_authenticated_user(state: &AppState, headers: &HeaderMap) -> Option<String> {
    let config = state.config.basic_auth.as_ref()?;
    basic_credentials_ok(config, headers).then(|| config.username.clone())
}

fn unauthorized_response() -> Response {
    let mut response = (
        StatusCode::UNAUTHORIZED,
        axum::Json(serde_json::json!({
            "error": { "code": "unauthorized", "message": "请先登录" }
        })),
    )
        .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn parse_basic_credentials(value: &str) -> Option<(String, String)> {
    let (scheme, encoded) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("Basic") {
        return None;
    }
    let decoded = STANDARD.decode(encoded.trim()).ok()?;
    let decoded = String::from_utf8(decoded).ok()?;
    let (username, password) = decoded.split_once(':')?;
    Some((username.to_string(), password.to_string()))
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.ct_eq(right).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(value: &str) -> String {
        STANDARD.encode(value.as_bytes())
    }

    #[test]
    fn parses_basic_authorization_header() {
        let value = format!("Basic {}", encode("alice:s3cret:with:colons"));
        assert_eq!(
            parse_basic_credentials(&value),
            Some(("alice".into(), "s3cret:with:colons".into()))
        );
        assert!(parse_basic_credentials("Bearer token").is_none());
        assert!(parse_basic_credentials("Basic !!!").is_none());
    }

    #[test]
    fn constant_time_comparison() {
        assert!(constant_time_eq(b"admin", b"admin"));
        assert!(!constant_time_eq(b"admin", b"admin2"));
        assert!(!constant_time_eq(b"admin", b"other"));
    }

    #[test]
    fn password_hash_roundtrip() {
        let salt = new_salt();
        let hash = hash_password("correct horse battery staple", &salt, 1_000);
        assert!(verify_password(
            "correct horse battery staple",
            &salt,
            1_000,
            &hash
        ));
        assert!(!verify_password("wrong password", &salt, 1_000, &hash));
        assert!(!verify_password(
            "correct horse battery staple",
            &new_salt(),
            1_000,
            &hash
        ));
    }

    #[test]
    fn credential_policy() {
        assert!(validate_credentials("admin", "password123").is_ok());
        assert!(validate_credentials("", "password123").is_err());
        assert!(validate_credentials("ad min", "password123").is_err());
        assert!(validate_credentials("admin", "short").is_err());
    }

    #[test]
    fn reads_session_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("theme=dark; fu_session=abc123; other=1"),
        );
        assert_eq!(session_token(&headers).as_deref(), Some("abc123"));
    }
}
