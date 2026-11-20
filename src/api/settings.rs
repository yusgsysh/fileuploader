use axum::{extract::State, response::IntoResponse, Json};

use crate::{error::AppError, models::AppSettings, state::AppState};

pub async fn get_settings(State(state): State<AppState>) -> impl IntoResponse {
    let settings = state.settings.read().await.clone();
    Json(settings)
}

pub async fn put_settings(
    State(state): State<AppState>,
    Json(settings): Json<AppSettings>,
) -> Result<impl IntoResponse, AppError> {
    let settings = state.manager.apply_settings(settings).await?;
    Ok(Json(settings))
}
