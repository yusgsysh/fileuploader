use axum::{
    extract::State,
    http::{header, HeaderMap, HeaderValue},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{auth, error::AppError, state::AppState};

#[derive(Debug, Clone, Serialize)]
pub struct AuthStatus {
    /// True when no account exists yet; the UI shows the initial setup form.
    pub setup_required: bool,
    pub authenticated: bool,
    pub basic_auth: bool,
    pub username: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Credentials {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangePassword {
    #[serde(default)]
    pub current_password: String,
    #[serde(default)]
    pub new_password: String,
}

pub async fn status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AuthStatus>, AppError> {
    let account = state.db.get_auth_account().await?;
    let username = current_username(&state, &headers).await;
    Ok(Json(AuthStatus {
        setup_required: account.is_none(),
        authenticated: username.is_some(),
        basic_auth: state.config.basic_auth.is_some(),
        username,
    }))
}

pub async fn setup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Credentials>,
) -> Result<Response, AppError> {
    if state.db.get_auth_account().await?.is_some() {
        return Err(AppError::conflict("管理员账号已存在，请直接登录"));
    }

    let username = body.username.trim().to_string();
    auth::validate_credentials(&username, &body.password)
        .map_err(|message| AppError::bad_request(message.to_string()))?;

    let salt = auth::new_salt();
    let password = body.password;
    let salt_for_hash = salt.clone();
    let password_hash = tokio::task::spawn_blocking(move || {
        auth::hash_password(&password, &salt_for_hash, auth::PASSWORD_ITERATIONS)
    })
    .await
    .map_err(|error| AppError::internal(format!("password hashing failed: {error}")))?;

    if let Err(error) = state
        .db
        .create_auth_account(
            &username,
            &password_hash,
            &salt,
            auth::PASSWORD_ITERATIONS as i64,
        )
        .await
    {
        if error.to_string().contains("UNIQUE") {
            return Err(AppError::conflict("管理员账号已存在，请直接登录"));
        }
        return Err(error.into());
    }

    Ok(start_session(&state, &headers, username, false).await)
}

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Credentials>,
) -> Result<Response, AppError> {
    let account = state
        .db
        .get_auth_account()
        .await?
        .ok_or_else(|| AppError::bad_request("尚未设置管理员账号"))?;

    if account.username != body.username.trim() {
        return Err(AppError::Unauthorized("用户名或密码错误".into()));
    }

    let password = body.password;
    let salt = account.salt.clone();
    let password_hash = account.password_hash.clone();
    let iterations = account.iterations.max(1) as u32;
    let valid = tokio::task::spawn_blocking(move || {
        auth::verify_password(&password, &salt, iterations, &password_hash)
    })
    .await
    .map_err(|error| AppError::internal(format!("password verification failed: {error}")))?;

    if !valid {
        return Err(AppError::Unauthorized("用户名或密码错误".into()));
    }

    let username = account.username;
    Ok(start_session(&state, &headers, username, false).await)
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(token) = auth::session_token(&headers) {
        state.sessions.remove(&token).await;
    }
    let (name, value) = auth::set_cookie_header(auth::clear_session_cookie());
    let mut response = Json(AuthStatus {
        setup_required: false,
        authenticated: false,
        basic_auth: state.config.basic_auth.is_some(),
        username: None,
    })
    .into_response();
    response.headers_mut().insert(name, value);
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

pub async fn change_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ChangePassword>,
) -> Result<Response, AppError> {
    let username = current_username(&state, &headers)
        .await
        .ok_or_else(|| AppError::Unauthorized("请先登录".into()))?;

    let account = state
        .db
        .get_auth_account()
        .await?
        .ok_or_else(|| AppError::bad_request("尚未设置管理员账号"))?;

    let current_password = body.current_password;
    let salt = account.salt.clone();
    let password_hash = account.password_hash.clone();
    let iterations = account.iterations.max(1) as u32;
    let valid = tokio::task::spawn_blocking(move || {
        auth::verify_password(&current_password, &salt, iterations, &password_hash)
    })
    .await
    .map_err(|error| AppError::internal(format!("password verification failed: {error}")))?;
    if !valid {
        return Err(AppError::Unauthorized("当前密码不正确".into()));
    }

    auth::validate_credentials(&username, &body.new_password)
        .map_err(|message| AppError::bad_request(message.to_string()))?;

    let new_salt = auth::new_salt();
    let new_password = body.new_password;
    let salt_for_hash = new_salt.clone();
    let new_hash = tokio::task::spawn_blocking(move || {
        auth::hash_password(&new_password, &salt_for_hash, auth::PASSWORD_ITERATIONS)
    })
    .await
    .map_err(|error| AppError::internal(format!("password hashing failed: {error}")))?;

    state
        .db
        .update_auth_password(
            &username,
            &new_hash,
            &new_salt,
            auth::PASSWORD_ITERATIONS as i64,
        )
        .await?;

    // Invalidate every existing session, then keep this device signed in.
    state.sessions.clear().await;
    Ok(start_session(&state, &headers, username, false).await)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn current_username(state: &AppState, headers: &HeaderMap) -> Option<String> {
    if let Some(token) = auth::session_token(headers) {
        if let Some(username) = state.sessions.validate(&token).await {
            return Some(username);
        }
    }
    auth::basic_authenticated_user(state, headers)
}

async fn start_session(
    state: &AppState,
    headers: &HeaderMap,
    username: String,
    setup_required: bool,
) -> Response {
    let token = auth::new_token();
    let secure = auth::request_is_secure(headers);
    state.sessions.insert(token.clone(), username.clone()).await;
    cookie_response(
        AuthStatus {
            setup_required,
            authenticated: true,
            basic_auth: state.config.basic_auth.is_some(),
            username: Some(username),
        },
        auth::session_cookie(&token, secure),
    )
}

fn cookie_response(status: AuthStatus, cookie: String) -> Response {
    let (name, value) = auth::set_cookie_header(cookie);
    let mut response = Json(status).into_response();
    response.headers_mut().insert(name, value);
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
