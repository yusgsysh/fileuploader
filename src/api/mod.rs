pub mod events;
pub mod s3;
pub mod settings;
pub mod system;
pub mod tasks;

use axum::{
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tasks", get(tasks::list_tasks).post(tasks::create_task))
        .route(
            "/tasks/{id}",
            get(tasks::get_task).delete(tasks::delete_task),
        )
        .route("/tasks/{id}/cancel", post(tasks::cancel_task))
        .route("/tasks/{id}/retry", post(tasks::retry_task))
        .route("/events", get(events::events))
        .route(
            "/settings",
            get(settings::get_settings).put(settings::put_settings),
        )
        .route("/s3/configs", get(s3::list_configs).post(s3::create_config))
        .route(
            "/s3/configs/{id}",
            put(s3::update_config).delete(s3::delete_config),
        )
        .route("/s3/buckets", get(s3::list_buckets))
        .route("/s3/test", post(s3::test_connection))
        .route("/system/status", get(system::system_status))
        .route("/system/mega/status", get(system::mega_status))
        .route("/system/mega/test", post(system::mega_test))
        .route("/healthz", get(system::healthz))
        .fallback(|| async {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": { "code": "not_found", "message": "API endpoint not found" }
                })),
            )
        })
}
