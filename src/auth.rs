use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use subtle::ConstantTimeEq;

use crate::state::AppState;

/// Optional HTTP Basic authentication for the whole application.
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

    let credentials = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_basic_credentials);

    if let Some((username, password)) = credentials {
        let username_ok = constant_time_eq(username.as_bytes(), config.username.as_bytes());
        let password_ok = constant_time_eq(password.as_bytes(), config.password.as_bytes());
        if username_ok & password_ok {
            return next.run(request).await;
        }
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
}
