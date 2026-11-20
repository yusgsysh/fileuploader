use std::path::Path;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;

use crate::{
    error::AppError,
    models::{MegaStatus, MegaTestResult, RuntimeSummary, SystemStatus},
    services::mega,
    state::AppState,
};

#[derive(Debug, Deserialize, Default)]
pub struct OptionalMegaTest {
    #[serde(default)]
    pub url: Option<String>,
}

pub async fn system_status(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let settings = state.settings.read().await.clone();
    let counts = state.db.task_counts().await?;
    let db_ok = sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&state.db.pool)
        .await
        .is_ok();
    let mega_status = state.mega.status(&settings).await;
    let (temp_bytes, temp_dirs) = mega::temp_usage(Path::new(&settings.mega_temp_dir));
    let status = SystemStatus {
        version: state.version.to_string(),
        uptime_secs: state.started_at.elapsed().as_secs(),
        database_ok: db_ok,
        queued_tasks: counts.pending.max(0) as usize,
        active_tasks: state.manager.active_count(),
        tasks: counts,
        mega_temp_bytes: temp_bytes,
        mega_temp_dirs: temp_dirs,
        settings: RuntimeSummary {
            max_http_tasks: settings.max_http_tasks,
            max_mega_tasks: settings.max_mega_tasks,
            upload_concurrency: settings.upload_concurrency,
            part_size_bytes: settings.part_size_bytes,
            http_buffer_bytes: settings.http_buffer_bytes,
            max_pending_parts: settings.max_pending_parts,
        },
        mega: mega_status,
    };
    Ok(Json(status))
}

pub async fn mega_status(State(state): State<AppState>) -> impl IntoResponse {
    let settings = state.settings.read().await.clone();
    let status: MegaStatus = state.mega.status(&settings).await;
    Json(status)
}

pub async fn mega_test(
    State(state): State<AppState>,
    payload: Option<Json<OptionalMegaTest>>,
) -> impl IntoResponse {
    let settings = state.settings.read().await.clone();
    let url = payload.and_then(|Json(payload)| payload.url);
    let result: MegaTestResult = state.mega.test(&settings, url.as_deref()).await;
    Json(result)
}

pub async fn healthz(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(&state.db.pool)
        .await
    {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))),
        Err(err) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "status": "error", "detail": err.to_string() })),
        ),
    }
}
