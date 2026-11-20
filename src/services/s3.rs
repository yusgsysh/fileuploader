use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use aws_config::retry::RetryConfig;
use aws_credential_types::Credentials;
use aws_sdk_s3::{
    config::{Region, RequestChecksumCalculation, ResponseChecksumValidation},
    primitives::ByteStream,
    types::{CompletedMultipartUpload, CompletedPart},
    Client,
};
use bytes::Bytes;
use tokio::{
    sync::mpsc,
    task::JoinSet,
    time::{sleep, timeout},
};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::{
    models::{parse_metadata, PartInfo, S3ConfigRecord},
    progress::ProgressHandle,
};

#[derive(Debug, thiserror::Error)]
pub enum S3Error {
    #[error("Upload cancelled")]
    Cancelled,
    #[error("Object already exists: {0}")]
    ObjectExists(String),
    #[error("Invalid S3 configuration: {0}")]
    Config(String),
    #[error("S3 operation failed: {0}")]
    Failed(String),
    #[error("Upload source failed: {0}")]
    Source(String),
    #[error("Object size verification failed: expected {expected} bytes, found {actual}")]
    SizeMismatch { expected: i64, actual: i64 },
}

impl S3Error {
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }
}

type KeyLock = Arc<tokio::sync::Mutex<()>>;

#[derive(Clone)]
pub struct S3Service {
    cache: Arc<Mutex<HashMap<String, CachedClient>>>,
    key_locks: Arc<Mutex<HashMap<String, std::sync::Weak<tokio::sync::Mutex<()>>>>>,
}

#[derive(Clone)]
struct CachedClient {
    updated_at: String,
    client: Client,
}

#[derive(Debug, Clone)]
pub struct UploadRequest {
    pub bucket: String,
    pub key: String,
    pub content_type: Option<String>,
    pub expected_size: Option<i64>,
    pub allow_overwrite: bool,
    pub metadata: HashMap<String, String>,
    pub task_id: String,
}

#[derive(Debug, Clone)]
pub struct UploadOutcome {
    pub uploaded_bytes: i64,
    pub parts: usize,
    pub etag: Option<String>,
}

#[derive(Debug)]
pub enum StreamPart {
    Part { number: i32, data: Bytes },
    Finish { total_bytes: i64 },
    Failed { message: String },
}

impl S3Service {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            key_locks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn client(&self, cfg: &S3ConfigRecord) -> Result<Client, S3Error> {
        if let Some(cached) = self.cache.lock().unwrap().get(&cfg.id) {
            if cached.updated_at == cfg.updated_at {
                return Ok(cached.client.clone());
            }
        }
        let endpoint = normalize_endpoint(cfg)?;
        let mut builder = aws_sdk_s3::Config::builder()
            .behavior_version(aws_config::BehaviorVersion::latest())
            .region(Region::new(cfg.region.clone()))
            .credentials_provider(Credentials::new(
                cfg.access_key.clone(),
                cfg.secret_key.clone(),
                None,
                None,
                "fileuploader",
            ))
            .force_path_style(cfg.path_style)
            // Some S3-compatible providers reject or mishandle the flexible
            // checksum trailers that recent AWS SDKs add by default.
            .request_checksum_calculation(RequestChecksumCalculation::WhenRequired)
            .response_checksum_validation(ResponseChecksumValidation::WhenRequired)
            // Retries are implemented by our upload engine so the configured
            // retry count is the actual maximum end-to-end.
            .retry_config(RetryConfig::disabled());
        if let Some(endpoint) = endpoint {
            builder = builder.endpoint_url(endpoint);
        }
        let client = Client::from_conf(builder.build());
        self.cache.lock().unwrap().insert(
            cfg.id.clone(),
            CachedClient {
                updated_at: cfg.updated_at.clone(),
                client: client.clone(),
            },
        );
        Ok(client)
    }

    pub fn invalidate(&self, id: &str) {
        self.cache.lock().unwrap().remove(id);
    }

    fn key_lock(&self, key: &str) -> KeyLock {
        let mut locks = self.key_locks.lock().unwrap();
        if locks.len() > 4096 {
            locks.retain(|_, weak| weak.strong_count() > 0);
        }
        if let Some(existing) = locks.get(key).and_then(std::sync::Weak::upgrade) {
            return existing;
        }
        let lock = Arc::new(tokio::sync::Mutex::new(()));
        locks.insert(key.to_string(), Arc::downgrade(&lock));
        lock
    }

    pub async fn test_connection(&self, cfg: &S3ConfigRecord) -> Result<(usize, u128), S3Error> {
        let client = self.client(cfg)?;
        let started = Instant::now();
        let response = timeout(Duration::from_secs(30), client.list_buckets().send())
            .await
            .map_err(|_| S3Error::Failed("S3 connection test timed out".into()))?
            .map_err(|err| S3Error::Failed(format!("{err:?}")))?;
        let count = response.buckets().len();
        Ok((count, started.elapsed().as_millis()))
    }

    pub async fn list_buckets(&self, cfg: &S3ConfigRecord) -> Result<Vec<String>, S3Error> {
        let client = self.client(cfg)?;
        let response = timeout(Duration::from_secs(30), client.list_buckets().send())
            .await
            .map_err(|_| S3Error::Failed("S3 bucket listing timed out".into()))?
            .map_err(|err| S3Error::Failed(format!("{err:?}")))?;
        let mut buckets = response
            .buckets()
            .iter()
            .filter_map(|bucket| bucket.name().map(ToOwned::to_owned))
            .collect::<Vec<_>>();
        buckets.sort();
        Ok(buckets)
    }

    async fn object_metadata(
        &self,
        client: &Client,
        bucket: &str,
        key: &str,
    ) -> Result<Option<HashMap<String, String>>, S3Error> {
        match client.head_object().bucket(bucket).key(key).send().await {
            Ok(response) => Ok(Some(response.metadata().cloned().unwrap_or_default())),
            Err(err) => {
                if err
                    .as_service_error()
                    .map(|service| service.is_not_found())
                    .unwrap_or(false)
                {
                    Ok(None)
                } else {
                    Err(S3Error::Failed(format!(
                        "HeadObject failed before upload: {err:?}"
                    )))
                }
            }
        }
    }

    async fn object_exists(
        &self,
        client: &Client,
        bucket: &str,
        key: &str,
    ) -> Result<bool, S3Error> {
        Ok(self.object_metadata(client, bucket, key).await?.is_some())
    }

    pub async fn abort(
        &self,
        cfg: &S3ConfigRecord,
        bucket: &str,
        key: &str,
        upload_id: &str,
        retries: u32,
    ) -> Result<(), S3Error> {
        let client = self.client(cfg)?;
        self.abort_upload(
            &client,
            bucket,
            key,
            upload_id,
            retries,
            &CancellationToken::new(),
        )
        .await
    }

    /// Consume a bounded part stream and upload it with S3 multipart upload.
    /// The stream sender is responsible for backpressure; this function never
    /// buffers more than `upload_concurrency` parts plus the channel capacity.
    #[allow(clippy::too_many_arguments)]
    pub async fn upload_stream(
        &self,
        cfg: &S3ConfigRecord,
        request: UploadRequest,
        mut receiver: mpsc::Receiver<StreamPart>,
        cancel: CancellationToken,
        progress: ProgressHandle,
        upload_concurrency: usize,
        max_retries: u32,
        part_size: i64,
    ) -> Result<UploadOutcome, S3Error> {
        const MAX_S3_OBJECT_BYTES: i64 = 5 * 1024 * 1024 * 1024 * 1024;
        const MIN_PART_BYTES: i64 = 5 * 1024 * 1024;
        const MAX_PART_BYTES: i64 = 5 * 1024 * 1024 * 1024;
        let client = self.client(cfg)?;
        if request.bucket.trim().is_empty() || request.key.trim().is_empty() {
            return Err(S3Error::Config("bucket and object key are required".into()));
        }
        if !(MIN_PART_BYTES..=MAX_PART_BYTES).contains(&part_size) {
            return Err(S3Error::Config(
                "Part size must be between 5 MiB and 5 GiB".into(),
            ));
        }
        if let Some(expected) = request.expected_size {
            if expected > MAX_S3_OBJECT_BYTES {
                return Err(S3Error::Config(
                    "Object exceeds the S3 maximum object size of 5 TiB".into(),
                ));
            }
        }
        // Serialize uploads to the same object inside this process. S3 does
        // not provide an atomic create-if-absent for multipart complete, so
        // this closes the race between two tasks targeting the same key.
        let object_lock = self.key_lock(&format!("{}|{}|{}", cfg.id, request.bucket, request.key));
        let _object_guard = tokio::select! {
            _ = cancel.cancelled() => return Err(S3Error::Cancelled),
            guard = object_lock.lock_owned() => guard,
        };

        if !request.allow_overwrite {
            if let Some(metadata) = self
                .object_metadata(&client, &request.bucket, &request.key)
                .await?
            {
                let same_task = metadata
                    .get("fileuploader-task-id")
                    .map(|owner| owner == &request.task_id)
                    .unwrap_or(false);
                if !same_task {
                    return Err(S3Error::ObjectExists(format!(
                        "s3://{}/{}",
                        request.bucket, request.key
                    )));
                }
            }
        }

        let planned_parts = request
            .expected_size
            .filter(|total| *total > 0 && part_size > 0)
            .map(|total| ((total + part_size - 1) / part_size) as usize);
        if let Some(planned) = planned_parts {
            progress.set_parts_total(Some(planned));
        }

        let mut upload_id: Option<String> = None;
        let result = self
            .upload_inner(
                &client,
                cfg,
                &request,
                &mut receiver,
                &cancel,
                &progress,
                upload_concurrency.max(1),
                max_retries,
                part_size,
                planned_parts,
                &mut upload_id,
            )
            .await;

        if result.is_err() {
            if let Some(id) = upload_id.as_deref() {
                warn!(
                    bucket = %request.bucket,
                    object = %request.key,
                    "aborting multipart upload after failure/cancellation"
                );
                match self
                    .abort_upload(
                        &client,
                        &request.bucket,
                        &request.key,
                        id,
                        max_retries,
                        &cancel,
                    )
                    .await
                {
                    Ok(()) => progress.set_upload(None, None),
                    Err(err) => warn!(
                        error = %err,
                        "failed to abort multipart upload; the upload ID is kept for retry"
                    ),
                }
            }
        }
        result
    }

    #[allow(clippy::too_many_arguments)]
    async fn upload_inner(
        &self,
        client: &Client,
        cfg: &S3ConfigRecord,
        request: &UploadRequest,
        receiver: &mut mpsc::Receiver<StreamPart>,
        cancel: &CancellationToken,
        progress: &ProgressHandle,
        concurrency: usize,
        max_retries: u32,
        part_size: i64,
        planned_parts: Option<usize>,
        upload_id: &mut Option<String>,
    ) -> Result<UploadOutcome, S3Error> {
        let mut metadata = if request.metadata.is_empty() {
            parse_metadata(&cfg.metadata_json)
        } else {
            request.metadata.clone()
        };
        metadata.insert("fileuploader-task-id".into(), request.task_id.clone());
        let mut join_set: JoinSet<Result<(i32, i64, String), S3Error>> = JoinSet::new();
        let mut completed: Vec<(i32, String)> = Vec::new();
        let mut expected_parts = 0usize;
        let mut bytes_seen = 0i64;
        let mut saw_finish = false;
        let mut next_part_number = 1i32;

        loop {
            if saw_finish && join_set.is_empty() {
                break;
            }
            if !saw_finish && join_set.len() >= concurrency {
                tokio::select! {
                    _ = cancel.cancelled() => return Err(S3Error::Cancelled),
                    joined = join_set.join_next() => {
                        if let Some(result) = joined {
                            let (number, size, etag) = result.map_err(|err| {
                                S3Error::Failed(format!("Upload worker panicked: {err}"))
                            })??;
                            progress.part_completed(PartInfo {
                                part_number: number,
                                size,
                                etag: Some(etag.clone()),
                                status: "uploaded".into(),
                            });
                            progress.add_uploaded(size as u64);
                            completed.push((number, etag));
                        }
                    }
                }
                continue;
            }
            if saw_finish {
                tokio::select! {
                    _ = cancel.cancelled() => return Err(S3Error::Cancelled),
                    joined = join_set.join_next() => {
                        if let Some(result) = joined {
                            let (number, size, etag) = result.map_err(|err| {
                                S3Error::Failed(format!("Upload worker panicked: {err}"))
                            })??;
                            progress.part_completed(PartInfo {
                                part_number: number,
                                size,
                                etag: Some(etag.clone()),
                                status: "uploaded".into(),
                            });
                            progress.add_uploaded(size as u64);
                            completed.push((number, etag));
                        }
                    }
                }
                continue;
            }

            tokio::select! {
                _ = cancel.cancelled() => return Err(S3Error::Cancelled),
                joined = join_set.join_next(), if !join_set.is_empty() => {
                    if let Some(result) = joined {
                        let (number, size, etag) = result.map_err(|err| {
                            S3Error::Failed(format!("Upload worker panicked: {err}"))
                        })??;
                        progress.part_completed(PartInfo {
                            part_number: number,
                            size,
                            etag: Some(etag.clone()),
                            status: "uploaded".into(),
                        });
                        progress.add_uploaded(size as u64);
                        completed.push((number, etag));
                    }
                }
                message = receiver.recv() => {
                    match message {
                        Some(StreamPart::Part { number, data }) => {
                            if number != next_part_number {
                                return Err(S3Error::Source(format!(
                                    "Part numbering error: expected {next_part_number}, got {number}"
                                )));
                            }
                            if number > 10_000 {
                                return Err(S3Error::Failed(
                                    "S3 multipart upload cannot exceed 10,000 parts; increase Part Size".into(),
                                ));
                            }
                            if data.is_empty() {
                                return Err(S3Error::Source("Received an empty multipart part".into()));
                            }
                            next_part_number += 1;
                            bytes_seen += data.len() as i64;
                            if bytes_seen > part_size * 10_000 {
                                return Err(S3Error::Failed(
                                    "Stream exceeds the maximum object size implied by Part Size".into(),
                                ));
                            }
                            if bytes_seen > 5 * 1024 * 1024 * 1024 * 1024 {
                                return Err(S3Error::Failed(
                                    "Object exceeds the S3 maximum object size of 5 TiB".into(),
                                ));
                            }
                            expected_parts += 1;
                            if planned_parts.is_none() {
                                progress.set_parts_total(Some(expected_parts));
                            }
                            if upload_id.is_none() {
                                let id = self
                                    .create_multipart(client, request, &metadata, max_retries, cancel)
                                    .await?;
                                progress.set_upload(Some(id.clone()), Some(request.key.clone()));
                                *upload_id = Some(id);
                            }
                            let id = upload_id.clone().unwrap_or_default();
                            let client = client.clone();
                            let bucket = request.bucket.clone();
                            let key = request.key.clone();
                            let cancel_task = cancel.clone();
                            let part_len = data.len() as i64;
                            join_set.spawn(async move {
                                let result = upload_part_with_retry(
                                    &client,
                                    &bucket,
                                    &key,
                                    &id,
                                    number,
                                    data,
                                    max_retries,
                                    &cancel_task,
                                )
                                .await?;
                                Ok((number, part_len, result))
                            });
                        }
                        Some(StreamPart::Finish { total_bytes }) => {
                            saw_finish = true;
                            if total_bytes < 0 {
                                return Err(S3Error::Source("Invalid source size".into()));
                            }
                            if total_bytes != bytes_seen {
                                return Err(S3Error::Source(format!(
                                    "Downloaded byte count mismatch: stream says {total_bytes}, received {bytes_seen}"
                                )));
                            }
                            if let Some(expected) = request.expected_size {
                                if total_bytes != expected {
                                    return Err(S3Error::SizeMismatch {
                                        expected,
                                        actual: total_bytes,
                                    });
                                }
                            }
                            if total_bytes == 0 {
                                // Zero-byte objects cannot be completed through
                                // multipart upload on all S3 implementations.
                                return self
                                    .put_empty(client, request, &metadata, max_retries, cancel, progress)
                                    .await;
                            }
                            if upload_id.is_none() {
                                return Err(S3Error::Source(
                                    "Download finished without producing any data".into(),
                                ));
                            }
                        }
                        Some(StreamPart::Failed { message }) => {
                            return Err(S3Error::Source(message));
                        }
                        None => {
                            if !saw_finish {
                                return Err(S3Error::Source(
                                    "Download stream ended without a completion signal".into(),
                                ));
                            }
                            saw_finish = true;
                        }
                    }
                }
            }
        }

        if !saw_finish {
            return Err(S3Error::Source("Upload stream ended unexpectedly".into()));
        }
        if completed.len() != expected_parts {
            return Err(S3Error::Failed(format!(
                "Only {} of {} parts completed",
                completed.len(),
                expected_parts
            )));
        }
        completed.sort_by_key(|(number, _)| *number);
        let id = upload_id
            .clone()
            .ok_or_else(|| S3Error::Failed("Multipart upload ID missing".into()))?;
        let etag = self
            .complete_multipart(
                client,
                &request.bucket,
                &request.key,
                &id,
                &completed,
                max_retries,
                cancel,
            )
            .await?;
        self.verify_size(client, &request.bucket, &request.key, bytes_seen)
            .await?;
        info!(
            bucket = %request.bucket,
            object = %request.key,
            bytes = bytes_seen,
            parts = completed.len(),
            "multipart upload completed"
        );
        Ok(UploadOutcome {
            uploaded_bytes: bytes_seen,
            parts: completed.len(),
            etag,
        })
    }

    async fn create_multipart(
        &self,
        client: &Client,
        request: &UploadRequest,
        metadata: &HashMap<String, String>,
        retries: u32,
        cancel: &CancellationToken,
    ) -> Result<String, S3Error> {
        let mut last_error = String::new();
        for attempt in 0..=retries {
            if cancel.is_cancelled() {
                return Err(S3Error::Cancelled);
            }
            let mut builder = client
                .create_multipart_upload()
                .bucket(&request.bucket)
                .key(&request.key)
                .set_metadata(Some(metadata.clone()));
            if let Some(content_type) = &request.content_type {
                builder = builder.content_type(content_type);
            }
            let result = tokio::select! {
                _ = cancel.cancelled() => return Err(S3Error::Cancelled),
                result = timeout(Duration::from_secs(30), builder.send()) => result,
            };
            match result {
                Ok(Ok(response)) => {
                    return response.upload_id().map(ToOwned::to_owned).ok_or_else(|| {
                        S3Error::Failed("CreateMultipartUpload returned no upload ID".into())
                    });
                }
                Ok(Err(err)) => last_error = format!("{err:?}"),
                Err(_) => last_error = "CreateMultipartUpload timed out".into(),
            }
            if attempt < retries {
                retry_delay(attempt, cancel).await?;
            }
        }
        Err(S3Error::Failed(format!(
            "CreateMultipartUpload failed: {last_error}"
        )))
    }

    async fn put_empty(
        &self,
        client: &Client,
        request: &UploadRequest,
        metadata: &HashMap<String, String>,
        retries: u32,
        cancel: &CancellationToken,
        _progress: &ProgressHandle,
    ) -> Result<UploadOutcome, S3Error> {
        let mut last_error = String::new();
        for attempt in 0..=retries {
            if cancel.is_cancelled() {
                return Err(S3Error::Cancelled);
            }
            let mut builder = client
                .put_object()
                .bucket(&request.bucket)
                .key(&request.key)
                .body(ByteStream::from(Bytes::new()))
                .set_metadata(Some(metadata.clone()));
            if let Some(content_type) = &request.content_type {
                builder = builder.content_type(content_type);
            }
            let result = tokio::select! {
                _ = cancel.cancelled() => return Err(S3Error::Cancelled),
                result = timeout(Duration::from_secs(30), builder.send()) => result,
            };
            match result {
                Ok(Ok(response)) => {
                    self.verify_size(client, &request.bucket, &request.key, 0)
                        .await?;
                    return Ok(UploadOutcome {
                        uploaded_bytes: 0,
                        parts: 0,
                        etag: response.e_tag().map(ToOwned::to_owned),
                    });
                }
                Ok(Err(err)) => last_error = format!("{err:?}"),
                Err(_) => last_error = "PutObject timed out".into(),
            }
            if attempt < retries {
                retry_delay(attempt, cancel).await?;
            }
        }
        Err(S3Error::Failed(format!(
            "Uploading empty object failed: {last_error}"
        )))
    }

    #[allow(clippy::too_many_arguments)]
    async fn complete_multipart(
        &self,
        client: &Client,
        bucket: &str,
        key: &str,
        upload_id: &str,
        completed: &[(i32, String)],
        retries: u32,
        cancel: &CancellationToken,
    ) -> Result<Option<String>, S3Error> {
        let parts = completed
            .iter()
            .map(|(number, etag)| {
                CompletedPart::builder()
                    .part_number(*number)
                    .e_tag(etag)
                    .build()
            })
            .collect::<Vec<_>>();
        let upload = CompletedMultipartUpload::builder()
            .set_parts(Some(parts))
            .build();
        let mut last_error = String::new();
        for attempt in 0..=retries {
            if cancel.is_cancelled() {
                return Err(S3Error::Cancelled);
            }
            let result = tokio::select! {
                _ = cancel.cancelled() => return Err(S3Error::Cancelled),
                result = timeout(
                    Duration::from_secs(120),
                    client
                        .complete_multipart_upload()
                        .bucket(bucket)
                        .key(key)
                        .upload_id(upload_id)
                        .multipart_upload(upload.clone())
                        .send(),
                ) => result,
            };
            match result {
                Ok(Ok(response)) => return Ok(response.e_tag().map(ToOwned::to_owned)),
                Ok(Err(err)) => last_error = format!("{err:?}"),
                Err(_) => last_error = "CompleteMultipartUpload timed out".into(),
            }
            if attempt < retries {
                retry_delay(attempt, cancel).await?;
            }
        }
        // A lost response after a successful complete can make a retry return
        // NoSuchUpload. If the object is now present we treat that as success.
        if self
            .object_exists(client, bucket, key)
            .await
            .unwrap_or(false)
        {
            return Ok(None);
        }
        Err(S3Error::Failed(format!(
            "CompleteMultipartUpload failed: {last_error}"
        )))
    }

    async fn abort_upload(
        &self,
        client: &Client,
        bucket: &str,
        key: &str,
        upload_id: &str,
        retries: u32,
        cancel: &CancellationToken,
    ) -> Result<(), S3Error> {
        let mut last_error = String::new();
        for attempt in 0..=retries {
            if cancel.is_cancelled() && attempt == 0 {
                // Even on cancellation, make one immediate best-effort abort
                // before giving up.
            }
            match timeout(
                Duration::from_secs(30),
                client
                    .abort_multipart_upload()
                    .bucket(bucket)
                    .key(key)
                    .upload_id(upload_id)
                    .send(),
            )
            .await
            {
                Ok(Ok(_)) => return Ok(()),
                Ok(Err(err)) => {
                    if err
                        .as_service_error()
                        .map(|service| service.is_no_such_upload())
                        .unwrap_or(false)
                    {
                        return Ok(());
                    }
                    last_error = format!("{err:?}");
                }
                Err(_) => last_error = "AbortMultipartUpload timed out".into(),
            }
            if attempt < retries {
                sleep(Duration::from_millis(250 * (1u64 << attempt))).await;
            }
        }
        Err(S3Error::Failed(format!(
            "AbortMultipartUpload failed: {last_error}"
        )))
    }

    async fn verify_size(
        &self,
        client: &Client,
        bucket: &str,
        key: &str,
        expected: i64,
    ) -> Result<(), S3Error> {
        let response = timeout(
            Duration::from_secs(30),
            client.head_object().bucket(bucket).key(key).send(),
        )
        .await
        .map_err(|_| S3Error::Failed("Size verification timed out".into()))?
        .map_err(|err| S3Error::Failed(format!("HeadObject verification failed: {err:?}")))?;
        let actual = response.content_length().unwrap_or(-1);
        if actual != expected {
            return Err(S3Error::SizeMismatch { expected, actual });
        }
        Ok(())
    }
}

impl Default for S3Service {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::too_many_arguments)]
async fn upload_part_with_retry(
    client: &Client,
    bucket: &str,
    key: &str,
    upload_id: &str,
    part_number: i32,
    data: Bytes,
    retries: u32,
    cancel: &CancellationToken,
) -> Result<String, S3Error> {
    let mut last_error = String::new();
    for attempt in 0..=retries {
        if cancel.is_cancelled() {
            return Err(S3Error::Cancelled);
        }
        let body = ByteStream::from(data.clone());
        let result = tokio::select! {
            _ = cancel.cancelled() => return Err(S3Error::Cancelled),
            result = timeout(
                Duration::from_secs(600),
                client
                    .upload_part()
                    .bucket(bucket)
                    .key(key)
                    .upload_id(upload_id)
                    .part_number(part_number)
                    .body(body)
                    .send(),
            ) => result,
        };
        match result {
            Ok(Ok(response)) => {
                return response.e_tag().map(ToOwned::to_owned).ok_or_else(|| {
                    S3Error::Failed(format!("UploadPart {part_number} returned no ETag"))
                })
            }
            Ok(Err(err)) => last_error = format!("{err:?}"),
            Err(_) => last_error = format!("UploadPart {part_number} timed out"),
        }
        if attempt < retries {
            retry_delay(attempt, cancel).await?;
        }
    }
    Err(S3Error::Failed(format!(
        "UploadPart {part_number} failed: {last_error}"
    )))
}

async fn retry_delay(attempt: u32, cancel: &CancellationToken) -> Result<(), S3Error> {
    let delay = Duration::from_millis(250 * (1u64 << attempt.min(6)));
    tokio::select! {
        _ = cancel.cancelled() => Err(S3Error::Cancelled),
        _ = sleep(delay) => Ok(()),
    }
}

fn normalize_endpoint(cfg: &S3ConfigRecord) -> Result<Option<String>, S3Error> {
    let endpoint = cfg.endpoint.trim();
    if endpoint.is_empty() {
        return Ok(None);
    }
    let with_scheme = if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        endpoint.to_string()
    } else {
        format!(
            "{}://{}",
            if cfg.use_tls { "https" } else { "http" },
            endpoint.trim_end_matches('/')
        )
    };
    let url = url::Url::parse(&with_scheme)
        .map_err(|_| S3Error::Config("S3 endpoint is not a valid URL".into()))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(S3Error::Config("S3 endpoint must use HTTP or HTTPS".into()));
    }
    if url.host_str().is_none() {
        return Err(S3Error::Config("S3 endpoint has no host".into()));
    }
    if cfg.use_tls && url.scheme() != "https" {
        return Err(S3Error::Config(
            "TLS is enabled but the S3 endpoint uses HTTP".into(),
        ));
    }
    Ok(Some(with_scheme.trim_end_matches('/').to_string()))
}
