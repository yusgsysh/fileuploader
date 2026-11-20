use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use url::Url;
use uuid::Uuid;

use crate::{
    db::now,
    error::AppError,
    models::{BucketInfo, S3ConfigDto, S3ConfigInput, S3ConfigRecord, S3TestResult},
    security::normalize_object_key,
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct BucketQuery {
    #[serde(default, alias = "config_id", alias = "s3_config_id")]
    pub config_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct TestRequest {
    #[serde(default, alias = "config_id", alias = "s3_config_id")]
    pub config_id: Option<String>,
    #[serde(default)]
    pub config: Option<S3ConfigInput>,
}

pub async fn list_configs(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let configs = state.db.list_s3_configs().await?;
    let dtos: Vec<S3ConfigDto> = configs.iter().map(S3ConfigDto::from).collect();
    Ok(Json(dtos))
}

pub async fn create_config(
    State(state): State<AppState>,
    Json(input): Json<S3ConfigInput>,
) -> Result<impl IntoResponse, AppError> {
    let record = build_config(input, None, None, true)?;
    state.db.insert_s3_config(&record).await?;
    Ok((StatusCode::CREATED, Json(S3ConfigDto::from(&record))))
}

pub async fn update_config(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<S3ConfigInput>,
) -> Result<impl IntoResponse, AppError> {
    let existing = state
        .db
        .get_s3_config(&id)
        .await?
        .ok_or_else(|| AppError::not_found("S3 configuration not found"))?;
    let record = build_config(input, Some(&existing), Some(&id), false)?;
    state.db.update_s3_config(&record).await?;
    state.s3.invalidate(&id);
    Ok(Json(S3ConfigDto::from(&record)))
}

pub async fn delete_config(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let _ = state
        .db
        .get_s3_config(&id)
        .await?
        .ok_or_else(|| AppError::not_found("S3 configuration not found"))?;
    if state.db.s3_config_in_use(&id).await? {
        return Err(AppError::conflict(
            "This S3 configuration is referenced by existing tasks and cannot be deleted",
        ));
    }
    state.db.delete_s3_config(&id).await?;
    state.s3.invalidate(&id);
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_buckets(
    State(state): State<AppState>,
    Query(query): Query<BucketQuery>,
) -> Result<impl IntoResponse, AppError> {
    let config = match query.config_id.as_deref().filter(|v| !v.is_empty()) {
        Some(id) => state
            .db
            .get_s3_config(id)
            .await?
            .ok_or_else(|| AppError::bad_request("S3 configuration not found"))?,
        None => state
            .db
            .get_default_s3_config()
            .await?
            .ok_or_else(|| AppError::bad_request("No S3 configuration exists"))?,
    };
    let buckets = state
        .s3
        .list_buckets(&config)
        .await
        .map_err(|e| AppError::Upstream(e.to_string()))?;
    let buckets: Vec<BucketInfo> = buckets
        .into_iter()
        .map(|name| BucketInfo {
            name,
            creation_date: None,
        })
        .collect();
    Ok(Json(buckets))
}

pub async fn test_connection(
    State(state): State<AppState>,
    Json(request): Json<TestRequest>,
) -> Result<impl IntoResponse, AppError> {
    let existing = match request.config_id.as_deref().filter(|v| !v.is_empty()) {
        Some(id) => Some(
            state
                .db
                .get_s3_config(id)
                .await?
                .ok_or_else(|| AppError::bad_request("S3 configuration not found"))?,
        ),
        None => None,
    };
    let config = match request.config {
        Some(input) => build_config(
            input,
            existing.as_ref(),
            existing.as_ref().map(|c| c.id.as_str()),
            existing.is_none(),
        )?,
        None => existing.ok_or_else(|| {
            AppError::bad_request("Provide a saved config_id or a full S3 configuration")
        })?,
    };
    match state.s3.test_connection(&config).await {
        Ok((count, latency)) => Ok(Json(S3TestResult {
            ok: true,
            message: format!("Connected successfully. {count} bucket(s) visible."),
            bucket_count: Some(count),
            latency_ms: latency,
        })),
        Err(err) => Ok(Json(S3TestResult {
            ok: false,
            message: err.to_string(),
            bucket_count: None,
            latency_ms: 0,
        })),
    }
}

fn build_config(
    input: S3ConfigInput,
    existing: Option<&S3ConfigRecord>,
    id: Option<&str>,
    require_secret: bool,
) -> Result<S3ConfigRecord, AppError> {
    let endpoint = input.endpoint.trim().trim_end_matches('/').to_string();
    if !endpoint.is_empty() {
        let candidate = if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
            endpoint.clone()
        } else {
            format!(
                "{}://{}",
                if input.use_tls { "https" } else { "http" },
                endpoint
            )
        };
        let url = Url::parse(&candidate)
            .map_err(|_| AppError::bad_request("S3 endpoint is not a valid URL"))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(AppError::bad_request("S3 endpoint must use HTTP or HTTPS"));
        }
        if input.use_tls && url.scheme() != "https" {
            return Err(AppError::bad_request(
                "TLS is enabled but the endpoint uses HTTP",
            ));
        }
    }
    let access_key = input.access_key.trim().to_string();
    if access_key.is_empty() {
        return Err(AppError::bad_request("S3 access key is required"));
    }
    let secret_key = input
        .secret_key
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| existing.map(|record| record.secret_key.clone()))
        .unwrap_or_default();
    if require_secret && secret_key.is_empty() {
        return Err(AppError::bad_request("S3 secret key is required"));
    }
    let bucket = input.bucket.trim().to_string();
    validate_bucket(&bucket)?;
    if input.region.trim().is_empty() {
        return Err(AppError::bad_request("S3 region is required"));
    }
    let default_prefix = if input.default_prefix.trim().is_empty() {
        String::new()
    } else {
        normalize_object_key(&input.default_prefix)
            .map_err(|e| AppError::bad_request(format!("Invalid default object path: {e}")))?
    };
    let metadata = validate_metadata(&input.metadata)?;
    let name = if input.name.trim().is_empty() {
        if !endpoint.is_empty() {
            endpoint
                .trim_start_matches("https://")
                .trim_start_matches("http://")
                .to_string()
        } else {
            "S3".to_string()
        }
    } else {
        input.name.trim().to_string()
    };
    let now = now();
    let id = id
        .map(ToOwned::to_owned)
        .or_else(|| existing.map(|record| record.id.clone()))
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    Ok(S3ConfigRecord {
        id,
        name,
        endpoint,
        region: input.region.trim().to_string(),
        access_key,
        secret_key,
        bucket,
        default_prefix,
        path_style: input.path_style,
        use_tls: input.use_tls,
        metadata_json: serde_json::to_string(&metadata)
            .map_err(|_| AppError::internal("failed to encode metadata"))?,
        created_at: existing
            .map(|record| record.created_at.clone())
            .unwrap_or_else(|| now.clone()),
        updated_at: now,
    })
}

fn validate_bucket(bucket: &str) -> Result<(), AppError> {
    if bucket.is_empty() {
        return Ok(());
    }
    if bucket.len() > 255
        || bucket
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '/' || c == '\\')
    {
        return Err(AppError::bad_request("Invalid bucket name"));
    }
    Ok(())
}

fn validate_metadata(
    metadata: &HashMap<String, String>,
) -> Result<HashMap<String, String>, AppError> {
    if metadata.len() > 32 {
        return Err(AppError::bad_request("Too many S3 metadata entries"));
    }
    let mut out = HashMap::new();
    for (key, value) in metadata {
        let key = key.trim().to_ascii_lowercase();
        if key.is_empty()
            || key.len() > 128
            || !key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(AppError::bad_request(format!(
                "Invalid S3 metadata key: {key}"
            )));
        }
        if value.len() > 2048 || value.chars().any(|c| c.is_control()) {
            return Err(AppError::bad_request(format!(
                "Invalid S3 metadata value for {key}"
            )));
        }
        out.insert(key, value.clone());
    }
    Ok(out)
}
