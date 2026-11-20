use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use crate::{error::AppError, models::CreateTaskRequest, state::AppState};

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub limit: Option<i64>,
}

pub async fn create_task(
    State(state): State<AppState>,
    Json(payload): Json<CreateTaskRequest>,
) -> Result<impl IntoResponse, AppError> {
    let record = state.manager.create_task(payload).await?;
    let dto = state.manager.task_dto(&record).await;
    Ok((StatusCode::CREATED, Json(dto)))
}

pub async fn list_tasks(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<impl IntoResponse, AppError> {
    let tasks = state.manager.list_tasks(query.limit.unwrap_or(200)).await?;
    Ok(Json(tasks))
}

pub async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let detail = state.manager.task_detail(&id).await?;
    Ok(Json(detail))
}

pub async fn cancel_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let task = state.manager.cancel_task(&id).await?;
    Ok(Json(task))
}

pub async fn retry_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let record = state.manager.retry_task(&id).await?;
    let dto = state.manager.task_dto(&record).await;
    Ok(Json(dto))
}

pub async fn delete_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    state.manager.delete_task(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}
