use std::collections::HashMap;

use bytes::Bytes;
use fileuploader::{
    db::Store,
    models::{S3ConfigRecord, TaskRecord, TaskStage, TaskStatus},
    progress::ProgressRegistry,
    services::s3::{S3Service, StreamPart, UploadRequest},
};
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

/// Optional S3 integration test. It runs against MinIO, R2, AWS or any
/// compatible S3 endpoint when the TEST_S3_* environment variables are set.
/// Without them it is a no-op so `cargo test` stays self-contained.
#[tokio::test]
async fn multipart_upload_and_size_verification() -> anyhow::Result<()> {
    let Some(endpoint) = std::env::var("TEST_S3_ENDPOINT").ok() else {
        eprintln!("skipping S3 integration test: TEST_S3_ENDPOINT is not set");
        return Ok(());
    };
    let access_key = env_or("TEST_S3_ACCESS_KEY", "minioadmin");
    let secret_key = env_or("TEST_S3_SECRET_KEY", "minioadmin");
    let bucket = env_or("TEST_S3_BUCKET", "downloads");
    let region = env_or("TEST_S3_REGION", "us-east-1");
    let path_style = env_or("TEST_S3_PATH_STYLE", "true") != "false";

    let config = S3ConfigRecord {
        id: "integration".into(),
        name: "Integration".into(),
        endpoint: endpoint.clone(),
        region,
        access_key,
        secret_key,
        bucket: bucket.clone(),
        default_prefix: String::new(),
        path_style,
        use_tls: endpoint.starts_with("https://"),
        metadata_json: "{}".into(),
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    let temp = tempfile::tempdir()?;
    let db_url = format!("sqlite://{}", temp.path().join("integration.db").display());
    let store = Store::connect(&db_url).await?;

    let service = S3Service::new();
    let client = service.client(&config).map_err(anyhow::Error::msg)?;
    let _ = client.create_bucket().bucket(&bucket).send().await;

    let task_id = Uuid::new_v4().to_string();
    let key = format!("fileuploader-integration/{}.bin", Uuid::new_v4());
    let now = chrono::Utc::now().to_rfc3339();
    let task = TaskRecord {
        id: task_id.clone(),
        url: "https://example.com/integration.bin".into(),
        source_type: "http".into(),
        resolved_source: Some("http".into()),
        s3_config_id: config.id.clone(),
        bucket: bucket.clone(),
        object_key: Some(key.clone()),
        filename: Some("integration.bin".into()),
        resolved_filename: Some("integration.bin".into()),
        content_type: Some("application/octet-stream".into()),
        total_bytes: Some(20 * 1024 * 1024),
        downloaded_bytes: 0,
        uploaded_bytes: 0,
        status: TaskStatus::Pending.as_str().into(),
        stage: TaskStage::Pending.as_str().into(),
        error: None,
        s3_upload_id: None,
        s3_key: None,
        temp_dir: None,
        temp_complete: false,
        user_agent: None,
        referer: None,
        cookie: None,
        headers_json: "{}".into(),
        auth_username: None,
        auth_password: None,
        mega_password: None,
        allow_overwrite: false,
        attempt: 1,
        created_at: now.clone(),
        updated_at: now,
        started_at: None,
        finished_at: None,
        range_supported: None,
        temp_bytes: 0,
    };
    store.insert_task(&task).await?;
    let (events, _) = broadcast::channel(32);
    let registry = ProgressRegistry::new(store.clone(), events);
    let progress = registry.create(task);

    let total = 20 * 1024 * 1024usize;
    let (tx, rx) = mpsc::channel(2);
    tokio::spawn(async move {
        let first = Bytes::from(vec![0x5a; 16 * 1024 * 1024]);
        let second = Bytes::from(vec![0xa5; 4 * 1024 * 1024]);
        tx.send(StreamPart::Part {
            number: 1,
            data: first,
        })
        .await
        .unwrap();
        tx.send(StreamPart::Part {
            number: 2,
            data: second,
        })
        .await
        .unwrap();
        tx.send(StreamPart::Finish {
            total_bytes: total as i64,
        })
        .await
        .unwrap();
    });

    let outcome = service
        .upload_stream(
            &config,
            UploadRequest {
                bucket: bucket.clone(),
                key: key.clone(),
                content_type: Some("application/octet-stream".into()),
                expected_size: Some(total as i64),
                allow_overwrite: false,
                metadata: HashMap::new(),
                task_id,
            },
            rx,
            CancellationToken::new(),
            progress,
            2,
            2,
            16 * 1024 * 1024,
        )
        .await?;
    assert_eq!(outcome.uploaded_bytes, total as i64);
    assert_eq!(outcome.parts, 2);

    let head = client
        .head_object()
        .bucket(&bucket)
        .key(&key)
        .send()
        .await?;
    assert_eq!(head.content_length(), Some(total as i64));
    client
        .delete_object()
        .bucket(&bucket)
        .key(&key)
        .send()
        .await?;
    Ok(())
}
