use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, RwLock as StdRwLock,
    },
    time::{Duration, SystemTime},
};

use axum::http::header::{HeaderName, HeaderValue};
use bytes::BytesMut;
use tokio::{
    fs::File,
    io::AsyncReadExt,
    sync::{mpsc, RwLock},
};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
use url::Url;
use uuid::Uuid;

use crate::{
    config::create_private_dir,
    db::{now, Store},
    error::AppError,
    models::{
        AppSettings, CreateTaskRequest, MegaProgress, ResolvedSource, S3ConfigRecord, SourceType,
        TaskDetailDto, TaskDto, TaskRecord, TaskStage, TaskStatus,
    },
    progress::{ProgressHandle, ProgressRegistry},
    security::{
        detect_source, mega_link_kind, normalize_object_key, sanitize_filename,
        validate_mega_share_url, validate_public_url,
    },
    services::{
        http::{HttpDownloader, HttpSourceSpec},
        mega::{self, MegaFile, MegaService},
        s3::{S3Service, StreamPart, UploadRequest},
    },
};

#[derive(Clone)]
struct RunningTask {
    token: CancellationToken,
    run_id: Uuid,
}

#[derive(Clone)]
pub struct TaskManager {
    inner: Arc<Inner>,
}

struct Inner {
    db: Store,
    settings: Arc<RwLock<AppSettings>>,
    s3: Arc<S3Service>,
    mega: Arc<MegaService>,
    progress: ProgressRegistry,
    tokens: StdRwLock<HashMap<String, RunningTask>>,
    gates: Gates,
    events: tokio::sync::broadcast::Sender<crate::models::ServerEvent>,
}

#[derive(Clone)]
struct Gates {
    total: Arc<DynamicGate>,
    http: Arc<DynamicGate>,
    mega: Arc<DynamicGate>,
}

struct DynamicGate {
    limit: AtomicUsize,
    active: AtomicUsize,
    notify: tokio::sync::Notify,
}

struct GatePermit {
    gate: Arc<DynamicGate>,
}

impl Drop for GatePermit {
    fn drop(&mut self) {
        self.gate.active.fetch_sub(1, Ordering::AcqRel);
        self.gate.notify.notify_waiters();
    }
}

impl DynamicGate {
    fn new(limit: usize) -> Arc<Self> {
        Arc::new(Self {
            limit: AtomicUsize::new(limit.max(1)),
            active: AtomicUsize::new(0),
            notify: tokio::sync::Notify::new(),
        })
    }

    fn set_limit(&self, limit: usize) {
        self.limit.store(limit.max(1), Ordering::Release);
        self.notify.notify_waiters();
    }

    fn try_acquire(self: &Arc<Self>) -> Option<GatePermit> {
        loop {
            let limit = self.limit.load(Ordering::Acquire);
            let active = self.active.load(Ordering::Acquire);
            if active >= limit {
                return None;
            }
            if self
                .active
                .compare_exchange_weak(active, active + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Some(GatePermit { gate: self.clone() });
            }
        }
    }

    async fn acquire(self: Arc<Self>, cancel: &CancellationToken) -> Option<GatePermit> {
        loop {
            if let Some(permit) = self.try_acquire() {
                return Some(permit);
            }
            let notified = self.notify.notified();
            if let Some(permit) = self.try_acquire() {
                return Some(permit);
            }
            tokio::select! {
                _ = cancel.cancelled() => return None,
                _ = notified => {}
            }
        }
    }
}

#[derive(Debug)]
struct TaskFailure {
    message: String,
    cancelled: bool,
}

impl TaskFailure {
    fn failed(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            cancelled: false,
        }
    }

    fn cancelled() -> Self {
        Self {
            message: "Cancelled by user".into(),
            cancelled: true,
        }
    }
}

impl TaskManager {
    pub async fn new(
        db: Store,
        settings: Arc<RwLock<AppSettings>>,
        s3: Arc<S3Service>,
        mega: Arc<MegaService>,
        events: tokio::sync::broadcast::Sender<crate::models::ServerEvent>,
    ) -> Self {
        let snapshot = settings.read().await.clone();
        let total = (snapshot.max_http_tasks + snapshot.max_mega_tasks).max(1) as usize;
        let progress = ProgressRegistry::new(db.clone(), events.clone());
        Self {
            inner: Arc::new(Inner {
                db,
                settings,
                s3,
                mega,
                progress,
                tokens: StdRwLock::new(HashMap::new()),
                gates: Gates {
                    total: DynamicGate::new(total),
                    http: DynamicGate::new(snapshot.max_http_tasks.max(1) as usize),
                    mega: DynamicGate::new(snapshot.max_mega_tasks.max(1) as usize),
                },
                events,
            }),
        }
    }

    pub fn settings(&self) -> Arc<RwLock<AppSettings>> {
        self.inner.settings.clone()
    }

    pub async fn create_task(&self, request: CreateTaskRequest) -> Result<TaskRecord, AppError> {
        let settings = self.inner.settings.read().await.clone();
        let queued = self.inner.db.count_nonterminal().await?;
        let hard_limit =
            settings.max_queued_tasks + settings.max_http_tasks + settings.max_mega_tasks;
        if queued >= hard_limit {
            return Err(AppError::TooManyRequests(format!(
                "Task queue is full ({queued} pending/running tasks). Wait for tasks to finish or raise the queue limit."
            )));
        }

        let url_text = request.url.trim().to_string();
        if url_text.is_empty() {
            return Err(AppError::bad_request("URL is required"));
        }
        if url_text.len() > crate::security::MAX_URL_CHARS {
            return Err(AppError::bad_request("URL is too long"));
        }
        let parsed_url = Url::parse(&url_text).map_err(|_| AppError::bad_request("Invalid URL"))?;
        let selected_source =
            detect_source(&parsed_url, request.source_type).map_err(AppError::bad_request)?;
        let resolved = match selected_source {
            SourceType::Mega => {
                validate_mega_share_url(&url_text).map_err(AppError::bad_request)?;
                ResolvedSource::Mega
            }
            _ => {
                validate_public_url(&url_text)
                    .await
                    .map_err(AppError::bad_request)?;
                ResolvedSource::Http
            }
        };

        let config = self
            .resolve_s3_config(request.s3_config_id.as_deref())
            .await?;
        let bucket = request
            .bucket
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| config.bucket.clone());
        validate_bucket(&bucket)?;
        let object_key = normalize_input_key(request.object_key.as_deref())?;
        let filename = request
            .filename
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(sanitize_filename);
        let headers = validate_headers(&request.headers)?;

        if let Some(password) = request.auth_password.as_deref() {
            if request.auth_username.as_deref().unwrap_or("").is_empty() {
                return Err(AppError::bad_request(
                    "Basic authentication password requires a username",
                ));
            }
            if password.len() > 4096 {
                return Err(AppError::bad_request(
                    "Basic authentication password is too long",
                ));
            }
        }
        if let Some(username) = request.auth_username.as_deref() {
            if username.len() > 512 {
                return Err(AppError::bad_request(
                    "Basic authentication username is too long",
                ));
            }
        }
        if let Some(user_agent) = request.user_agent.as_deref() {
            if user_agent.len() > 2048 {
                return Err(AppError::bad_request("User-Agent is too long"));
            }
        }
        if let Some(cookie) = request.cookie.as_deref() {
            if cookie.len() > 16_384 {
                return Err(AppError::bad_request("Cookie header is too long"));
            }
        }
        if let Some(referer) = request.referer.as_deref() {
            if referer.len() > crate::security::MAX_URL_CHARS {
                return Err(AppError::bad_request("Referer is too long"));
            }
        }

        let id = Uuid::new_v4().to_string();
        let timestamp = now();
        let task = TaskRecord {
            id: id.clone(),
            url: url_text,
            source_type: request.source_type.as_str().to_string(),
            resolved_source: Some(resolved.as_str().to_string()),
            s3_config_id: config.id.clone(),
            bucket,
            object_key,
            filename,
            resolved_filename: None,
            content_type: None,
            total_bytes: None,
            downloaded_bytes: 0,
            uploaded_bytes: 0,
            status: TaskStatus::Pending.as_str().to_string(),
            stage: TaskStage::Pending.as_str().to_string(),
            error: None,
            s3_upload_id: None,
            s3_key: None,
            temp_dir: None,
            temp_complete: false,
            user_agent: request.user_agent.clone().filter(|v| !v.trim().is_empty()),
            referer: request.referer.clone().filter(|v| !v.trim().is_empty()),
            cookie: request.cookie.clone().filter(|v| !v.trim().is_empty()),
            headers_json: serde_json::to_string(&headers)
                .map_err(|_| AppError::internal("failed to encode headers"))?,
            auth_username: request
                .auth_username
                .clone()
                .filter(|v| !v.trim().is_empty()),
            auth_password: request.auth_password.clone().filter(|v| !v.is_empty()),
            allow_overwrite: request.allow_overwrite,
            attempt: 1,
            created_at: timestamp.clone(),
            updated_at: timestamp,
            started_at: None,
            finished_at: None,
            range_supported: None,
            temp_bytes: 0,
        };
        self.inner.db.insert_task(&task).await?;
        let progress = self.inner.progress.create(task.clone());
        self.spawn_runner(id.clone());
        info!(
            task_id = %id,
            source = resolved.as_str(),
            bucket = %task.bucket,
            "task created"
        );
        let _ = progress;
        Ok(task)
    }

    pub async fn get_task(&self, id: &str) -> Result<TaskRecord, AppError> {
        self.inner
            .db
            .get_task(id)
            .await?
            .ok_or_else(|| AppError::not_found("Task not found"))
    }

    pub async fn task_dto(&self, record: &TaskRecord) -> TaskDto {
        self.inner.progress.dto_or_record(record)
    }

    pub async fn list_tasks(&self, limit: i64) -> Result<Vec<TaskDto>, AppError> {
        let records = self.inner.db.list_tasks(limit).await?;
        Ok(records
            .iter()
            .map(|record| self.inner.progress.summary_or_record(record))
            .collect())
    }

    pub async fn task_detail(&self, id: &str) -> Result<TaskDetailDto, AppError> {
        let record = self.get_task(id).await?;
        let task = self.inner.progress.dto_or_record(&record);
        let history = self.inner.db.list_history(id).await?;
        Ok(TaskDetailDto { task, history })
    }

    pub async fn cancel_task(&self, id: &str) -> Result<TaskDto, AppError> {
        let record = self.get_task(id).await?;
        if record.status_enum().is_terminal() {
            return Ok(self.inner.progress.dto_or_record(&record));
        }
        let running = self.inner.tokens.read().unwrap().get(id).cloned();
        if let Some(running) = running {
            running.token.cancel();
            info!(task_id = %id, "task cancellation requested");
            if let Some(progress) = self.inner.progress.get(id) {
                progress.set_error("Cancelling...");
            }
        } else if let Some(progress) = self.inner.progress.get(id) {
            progress
                .set_status_awaited(
                    TaskStatus::Cancelled,
                    TaskStage::Cancelled,
                    Some("Cancelled by user".into()),
                )
                .await;
        } else {
            self.inner
                .db
                .update_task_status(
                    id,
                    TaskStatus::Cancelled,
                    TaskStage::Cancelled,
                    Some("Cancelled by user"),
                    true,
                )
                .await?;
        }
        let record = self.get_task(id).await?;
        Ok(self.inner.progress.dto_or_record(&record))
    }

    pub async fn retry_task(&self, id: &str) -> Result<TaskRecord, AppError> {
        let mut record = self.get_task(id).await?;
        if record.status_enum().is_active() {
            return Err(AppError::conflict("Task is still active"));
        }
        let settings = self.inner.settings.read().await.clone();
        if let Some(upload_id) = record.s3_upload_id.clone() {
            if let Some(config) = self.inner.db.get_s3_config(&record.s3_config_id).await? {
                if let Err(err) = self
                    .inner
                    .s3
                    .abort(
                        &config,
                        &record.bucket,
                        record.s3_key.as_deref().unwrap_or_default(),
                        &upload_id,
                        settings.max_retries as u32,
                    )
                    .await
                {
                    return Err(AppError::Upstream(format!(
                        "Cannot retry until the previous multipart upload is aborted: {err}"
                    )));
                }
            }
        }
        self.inner
            .db
            .reset_task_for_retry(
                id,
                record.resolved_source.as_deref(),
                &record.s3_config_id,
                &record.bucket,
                record.object_key.as_deref(),
            )
            .await?;
        record = self.get_task(id).await?;
        self.inner.progress.remove(id);
        self.inner.progress.create(record.clone());
        self.spawn_runner(id.to_string());
        info!(task_id = %id, attempt = record.attempt, "task retry queued");
        Ok(record)
    }

    pub async fn delete_task(&self, id: &str) -> Result<(), AppError> {
        let record = self.get_task(id).await?;
        if record.status_enum().is_active() {
            return Err(AppError::conflict(
                "Cancel the running task before deleting it",
            ));
        }
        let settings = self.inner.settings.read().await.clone();
        if let (Some(upload_id), Ok(Some(config))) = (
            record.s3_upload_id.as_deref(),
            self.inner.db.get_s3_config(&record.s3_config_id).await,
        ) {
            if let Err(err) = self
                .inner
                .s3
                .abort(
                    &config,
                    &record.bucket,
                    record.s3_key.as_deref().unwrap_or_default(),
                    upload_id,
                    settings.max_retries as u32,
                )
                .await
            {
                warn!(task_id = %id, error = %err, "failed to abort multipart upload while deleting task");
            }
        }
        if let Ok(task_dir) = self.inner.mega.task_dir(&settings, id) {
            let _ = mega::cleanup_task_dir(&task_dir).await;
        }
        self.inner.db.delete_task(id).await?;
        self.inner.progress.remove(id);
        self.inner.tokens.write().unwrap().remove(id);
        let _ = self
            .inner
            .events
            .send(crate::models::ServerEvent::TaskDeleted {
                task_id: id.to_string(),
            });
        Ok(())
    }

    pub async fn apply_settings(&self, mut settings: AppSettings) -> Result<AppSettings, AppError> {
        settings.normalize_paths();
        settings.validate().map_err(AppError::bad_request)?;
        create_private_dir(std::path::Path::new(&settings.mega_home))
            .map_err(|e| AppError::internal(e.to_string()))?;
        create_private_dir(std::path::Path::new(&settings.mega_temp_dir))
            .map_err(|e| AppError::internal(e.to_string()))?;
        self.inner.db.save_settings(&settings).await?;
        *self.inner.settings.write().await = settings.clone();
        let total = settings.max_http_tasks + settings.max_mega_tasks;
        self.inner.gates.total.set_limit(total.max(1) as usize);
        self.inner
            .gates
            .http
            .set_limit(settings.max_http_tasks.max(1) as usize);
        self.inner
            .gates
            .mega
            .set_limit(settings.max_mega_tasks.max(1) as usize);
        info!("settings updated");
        Ok(settings)
    }

    pub async fn startup(&self) -> anyhow::Result<()> {
        let interrupted = self.inner.db.mark_interrupted_tasks().await?;
        if interrupted > 0 {
            warn!(count = interrupted, "marked interrupted tasks as failed");
        }
        self.cleanup_stale_temp().await;
        let manager = self.clone();
        let settings = self.inner.settings.read().await.clone();
        if settings.mega_auto_start_server {
            let mega = self.inner.mega.clone();
            let temp_root = std::fs::canonicalize(&settings.mega_temp_dir)
                .unwrap_or_else(|_| std::path::PathBuf::from(&settings.mega_temp_dir));
            tokio::spawn(async move {
                match mega.cancel_orphan_downloads(&settings, &temp_root).await {
                    Ok(0) => {}
                    Ok(count) => warn!(
                        count,
                        "cancelled orphan MEGA transfers left by a previous run"
                    ),
                    Err(err) => {
                        warn!(error = %err, "could not cancel orphan MEGA transfers at startup")
                    }
                }
            });
        }
        tokio::spawn(async move {
            manager.cleanup_orphan_multipart().await;
        });
        Ok(())
    }

    pub fn start_background(&self) {
        let manager = self.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_millis(500));
            let mut ticks: u64 = 0;
            loop {
                ticker.tick().await;
                manager.inner.progress.tick();
                ticks += 1;
                if ticks.is_multiple_of(7200) {
                    manager.cleanup_stale_temp().await;
                }
            }
        });
    }

    pub fn cancel_all(&self) {
        for running in self.inner.tokens.read().unwrap().values() {
            running.token.cancel();
        }
    }

    pub fn active_count(&self) -> usize {
        self.inner.progress.active_count()
    }

    fn spawn_runner(&self, id: String) {
        let token = CancellationToken::new();
        let run_id = Uuid::new_v4();
        self.inner.tokens.write().unwrap().insert(
            id.clone(),
            RunningTask {
                token: token.clone(),
                run_id,
            },
        );
        let manager = self.clone();
        tokio::spawn(async move {
            manager.run_task(id, token, run_id).await;
        });
    }

    async fn run_task(&self, id: String, token: CancellationToken, run_id: Uuid) {
        let total_permit = match self.inner.gates.total.clone().acquire(&token).await {
            Some(permit) => permit,
            None => {
                self.finalize_without_run(&id, run_id, "Cancelled while queued")
                    .await;
                return;
            }
        };
        let record = match self.inner.db.get_task(&id).await {
            Ok(Some(record)) => record,
            Ok(None) => return,
            Err(err) => {
                warn!(task_id = %id, error = %err, "failed to load queued task");
                return;
            }
        };
        if record.status_enum().is_terminal() {
            return;
        }
        let source = match record.resolved() {
            Some(source) => source,
            None => ResolvedSource::Http,
        };
        let source_gate = match source {
            ResolvedSource::Http => self.inner.gates.http.clone(),
            ResolvedSource::Mega => self.inner.gates.mega.clone(),
        };
        let source_permit = match source_gate.acquire(&token).await {
            Some(permit) => permit,
            None => {
                self.finalize_without_run(&id, run_id, "Cancelled while queued")
                    .await;
                return;
            }
        };
        if token.is_cancelled() {
            self.finalize_without_run(&id, run_id, "Cancelled while queued")
                .await;
            return;
        }

        let settings = self.inner.settings.read().await.clone();
        let progress = self
            .inner
            .progress
            .get(&id)
            .unwrap_or_else(|| self.inner.progress.create(record.clone()));
        let initial_stage = match source {
            ResolvedSource::Http => TaskStage::PreparingUpload,
            ResolvedSource::Mega => TaskStage::Downloading,
        };
        if let Err(err) = self.inner.db.mark_started(&id).await {
            warn!(task_id = %id, error = %err, "failed to mark task started");
        }
        progress.mark_started();
        progress.set_status(TaskStatus::Running, initial_stage, None);
        if let Err(err) = self
            .inner
            .db
            .insert_history_start(&id, record.attempt, "running", initial_stage.as_str())
            .await
        {
            warn!(task_id = %id, error = %err, "failed to write task history");
        }
        info!(task_id = %id, source = source.as_str(), "task started");

        let result = match source {
            ResolvedSource::Http => self.run_http(&record, &settings, &progress, &token).await,
            ResolvedSource::Mega => self.run_mega(&record, &settings, &progress, &token).await,
        };
        let cancelled =
            token.is_cancelled() || result.as_ref().err().map(|e| e.cancelled).unwrap_or(false);
        if cancelled {
            self.finish_task(
                &record,
                &settings,
                &progress,
                Err(TaskFailure::cancelled()),
                true,
            )
            .await;
        } else {
            self.finish_task(&record, &settings, &progress, result, false)
                .await;
        }
        self.remove_runner(&id, run_id);
        drop(source_permit);
        drop(total_permit);
    }

    fn remove_runner(&self, id: &str, run_id: Uuid) {
        let mut tokens = self.inner.tokens.write().unwrap();
        if tokens
            .get(id)
            .map(|running| running.run_id == run_id)
            .unwrap_or(false)
        {
            tokens.remove(id);
        }
    }

    async fn finalize_without_run(&self, id: &str, run_id: Uuid, message: &str) {
        if let Some(progress) = self.inner.progress.get(id) {
            progress
                .set_status_awaited(
                    TaskStatus::Cancelled,
                    TaskStage::Cancelled,
                    Some(message.to_string()),
                )
                .await;
        } else {
            let _ = self
                .inner
                .db
                .update_task_status(
                    id,
                    TaskStatus::Cancelled,
                    TaskStage::Cancelled,
                    Some(message),
                    true,
                )
                .await;
        }
        self.remove_runner(id, run_id);
    }

    async fn finish_task(
        &self,
        record: &TaskRecord,
        settings: &AppSettings,
        progress: &ProgressHandle,
        result: Result<(), TaskFailure>,
        cancelled: bool,
    ) {
        let succeeded = result.is_ok() && !cancelled;
        let mut cleanup_warning = None;
        if succeeded && record.resolved() == Some(ResolvedSource::Mega) {
            progress.set_stage(TaskStage::Finalizing);
            if let Ok(task_dir) = self.inner.mega.task_dir(settings, &record.id) {
                match mega::cleanup_task_dir(&task_dir).await {
                    Ok(()) => progress.clear_temp(),
                    Err(err) => {
                        cleanup_warning = Some(format!(
                            "Upload succeeded, but temporary files could not be removed: {err}"
                        ));
                        warn!(task_id = %record.id, error = %err, "temporary file cleanup failed");
                    }
                }
            }
        }

        let (status, stage, error) = if succeeded {
            (TaskStatus::Completed, TaskStage::Completed, cleanup_warning)
        } else if cancelled {
            (
                TaskStatus::Cancelled,
                TaskStage::Cancelled,
                Some("Cancelled by user".into()),
            )
        } else {
            let message = result
                .err()
                .map(|failure| failure.message)
                .unwrap_or_else(|| "Task failed".into());
            let message = truncate_error(&message, 4000);
            (TaskStatus::Failed, TaskStage::Failed, Some(message))
        };

        if succeeded {
            progress.set_upload(None, None);
        }
        progress.persist_counters();
        progress
            .set_status_awaited(status, stage, error.clone())
            .await;
        if let Err(err) = self
            .inner
            .db
            .finish_history(
                &record.id,
                record.attempt,
                status.as_str(),
                stage.as_str(),
                error.as_deref(),
            )
            .await
        {
            warn!(task_id = %record.id, error = %err, "failed to finish task history");
        }
        if succeeded {
            info!(task_id = %record.id, "task completed");
        } else if cancelled {
            info!(task_id = %record.id, "task cancelled");
        } else {
            warn!(
                task_id = %record.id,
                error = %error.clone().unwrap_or_default(),
                "task failed"
            );
        }
    }

    async fn run_http(
        &self,
        record: &TaskRecord,
        settings: &AppSettings,
        progress: &ProgressHandle,
        token: &CancellationToken,
    ) -> Result<(), TaskFailure> {
        progress.set_stage(TaskStage::PreparingUpload);
        let downloader =
            HttpDownloader::new(settings).map_err(|e| TaskFailure::failed(e.to_string()))?;
        let spec = HttpSourceSpec {
            url: record.url.clone(),
            user_agent: record.user_agent.clone(),
            referer: record.referer.clone(),
            cookie: record.cookie.clone(),
            headers: record.headers(),
            auth_username: record.auth_username.clone(),
            auth_password: record.auth_password.clone(),
        };
        let prepared = downloader
            .prepare(&spec, record.filename.as_deref(), token)
            .await
            .map_err(|e| {
                if e.is_cancelled() {
                    TaskFailure::cancelled()
                } else {
                    TaskFailure::failed(e.to_string())
                }
            })?;
        let info = prepared.info.clone();
        if let Some(total) = info.total_bytes {
            if settings.max_file_bytes > 0 && total > settings.max_file_bytes {
                return Err(TaskFailure::failed(format!(
                    "Source file is larger than the configured limit of {} bytes",
                    settings.max_file_bytes
                )));
            }
        }
        progress.set_source(
            ResolvedSource::Http,
            Some(info.filename.clone()),
            info.content_type.clone(),
            info.total_bytes,
            info.range_supported,
        );
        let config = self
            .inner
            .db
            .get_s3_config(&record.s3_config_id)
            .await
            .map_err(|e| TaskFailure::failed(e.to_string()))?
            .ok_or_else(|| TaskFailure::failed("S3 configuration no longer exists"))?;
        let key =
            single_object_key(&config, record, &info.filename).map_err(TaskFailure::failed)?;
        progress.set_upload(None, Some(key.clone()));

        let part_size = settings.part_size_bytes as usize;
        let (tx, rx) = mpsc::channel(settings.max_pending_parts.max(1) as usize);
        let pipeline_cancel = token.child_token();
        let downloader_cancel = pipeline_cancel.clone();
        let http_progress = progress.clone();
        let downloader_task = tokio::spawn(async move {
            downloader
                .stream(
                    prepared,
                    spec,
                    part_size,
                    tx,
                    http_progress,
                    downloader_cancel,
                )
                .await
        });
        progress.set_stage(TaskStage::Downloading);
        let upload_request = UploadRequest {
            bucket: record.bucket.clone(),
            key,
            content_type: info.content_type,
            expected_size: info.total_bytes,
            allow_overwrite: record.allow_overwrite,
            metadata: HashMap::new(),
            task_id: record.id.clone(),
        };
        let upload = self
            .inner
            .s3
            .upload_stream(
                &config,
                upload_request,
                rx,
                pipeline_cancel.clone(),
                progress.clone(),
                settings.upload_concurrency as usize,
                settings.max_retries as u32,
                settings.part_size_bytes,
            )
            .await;
        if upload.is_err() {
            pipeline_cancel.cancel();
        }
        let download = downloader_task.await;
        if token.is_cancelled() {
            return Err(TaskFailure::cancelled());
        }
        match upload {
            Ok(outcome) => {
                progress.set_uploaded(outcome.uploaded_bytes.max(0) as u64);
                match download {
                    Ok(Ok(())) => {
                        progress.set_stage(TaskStage::Finalizing);
                        Ok(())
                    }
                    Ok(Err(err)) => Err(TaskFailure::failed(err.to_string())),
                    Err(err) => Err(TaskFailure::failed(format!(
                        "download worker failed: {err}"
                    ))),
                }
            }
            Err(err) => {
                let _ = download;
                Err(TaskFailure::failed(err.to_string()))
            }
        }
    }

    async fn run_mega(
        &self,
        record: &TaskRecord,
        settings: &AppSettings,
        progress: &ProgressHandle,
        token: &CancellationToken,
    ) -> Result<(), TaskFailure> {
        progress.set_stage(TaskStage::Downloading);
        let parsed =
            Url::parse(&record.url).map_err(|_| TaskFailure::failed("Invalid MEGA URL"))?;
        let kind = mega_link_kind(&parsed);
        let download = match self
            .inner
            .mega
            .load_completed(settings, &record.id, progress)
            .map_err(|e| TaskFailure::failed(e.to_string()))?
        {
            Some(download) => download,
            None => self
                .inner
                .mega
                .download(
                    settings,
                    &record.url,
                    &record.id,
                    progress.clone(),
                    token.child_token(),
                )
                .await
                .map_err(|e| {
                    if e.is_cancelled() || token.is_cancelled() {
                        TaskFailure::cancelled()
                    } else {
                        TaskFailure::failed(e.to_string())
                    }
                })?,
        };
        let display_name = if download.files.len() == 1 {
            download.files[0]
                .relative_path
                .file_name()
                .map(|v| v.to_string_lossy().to_string())
                .unwrap_or_else(|| "MEGA file".into())
        } else {
            download
                .files
                .first()
                .and_then(|f| f.relative_path.components().next())
                .map(|c| format!("MEGA folder ({})", c.as_os_str().to_string_lossy()))
                .unwrap_or_else(|| "MEGA folder".into())
        };
        progress.set_source(
            ResolvedSource::Mega,
            Some(display_name),
            None,
            Some(download.total_bytes),
            None,
        );
        progress.set_total(Some(download.total_bytes));

        let config = self
            .inner
            .db
            .get_s3_config(&record.s3_config_id)
            .await
            .map_err(|e| TaskFailure::failed(e.to_string()))?
            .ok_or_else(|| TaskFailure::failed("S3 configuration no longer exists"))?;
        let mut keys = Vec::with_capacity(download.files.len());
        let mut unique = HashSet::new();
        for file in &download.files {
            let key = mega_object_key(&config, record, kind, file).map_err(TaskFailure::failed)?;
            if !unique.insert(key.clone()) {
                return Err(TaskFailure::failed(format!(
                    "Multiple MEGA files map to the same object key: {key}"
                )));
            }
            keys.push(key);
        }
        let total_files = download.files.len();
        progress.set_stage(TaskStage::Uploading);
        let mut uploaded_total = 0i64;
        for (index, (file, key)) in download.files.iter().zip(keys.iter()).enumerate() {
            if token.is_cancelled() {
                return Err(TaskFailure::cancelled());
            }
            progress.reset_parts();
            let expected_parts = if settings.part_size_bytes > 0 {
                Some(
                    ((file.size + settings.part_size_bytes - 1) / settings.part_size_bytes)
                        as usize,
                )
            } else {
                None
            };
            progress.set_parts_total(expected_parts);
            progress.set_mega(Some(MegaProgress {
                process_state: "uploading".into(),
                temp_bytes: mega::dir_size(&download.temp_dir),
                temp_dir: Some(download.temp_dir.to_string_lossy().to_string()),
                files_total: total_files,
                files_done: index,
                current_file: Some(file.relative_path.to_string_lossy().to_string()),
                percentage: Some(100.0),
            }));
            let (tx, rx) = mpsc::channel(settings.max_pending_parts.max(1) as usize);
            let file_cancel = token.child_token();
            let sender_cancel = file_cancel.clone();
            let path = file.local_path.clone();
            let expected_size = file.size;
            let file_part_size = settings.part_size_bytes as usize;
            let sender = tokio::spawn(async move {
                stream_local_file(&path, expected_size, file_part_size, tx, sender_cancel).await
            });
            let request = UploadRequest {
                bucket: record.bucket.clone(),
                key: key.clone(),
                content_type: file.content_type.clone(),
                expected_size: Some(file.size),
                allow_overwrite: record.allow_overwrite,
                metadata: HashMap::new(),
                task_id: record.id.clone(),
            };
            let upload = self
                .inner
                .s3
                .upload_stream(
                    &config,
                    request,
                    rx,
                    file_cancel.clone(),
                    progress.clone(),
                    settings.upload_concurrency as usize,
                    settings.max_retries as u32,
                    settings.part_size_bytes,
                )
                .await;
            if upload.is_err() {
                file_cancel.cancel();
            }
            let sender_result = sender.await;
            if token.is_cancelled() {
                return Err(TaskFailure::cancelled());
            }
            match upload {
                Ok(outcome) => {
                    uploaded_total += outcome.uploaded_bytes;
                    progress.set_uploaded(uploaded_total.max(0) as u64);
                    match sender_result {
                        Ok(Ok(())) => {}
                        Ok(Err(err)) => return Err(TaskFailure::failed(err)),
                        Err(err) => {
                            return Err(TaskFailure::failed(format!("file reader failed: {err}")))
                        }
                    }
                }
                Err(err) => {
                    let _ = sender_result;
                    return Err(TaskFailure::failed(err.to_string()));
                }
            }
        }
        if kind == "folder" {
            if let Some(prefix) = mega_folder_prefix(&config, record, &download.files) {
                progress.set_s3_key(Some(format!("{prefix}/")));
            }
        }
        progress.set_mega(Some(MegaProgress {
            process_state: "completed".into(),
            temp_bytes: mega::dir_size(&download.temp_dir),
            temp_dir: Some(download.temp_dir.to_string_lossy().to_string()),
            files_total: total_files,
            files_done: total_files,
            current_file: None,
            percentage: Some(100.0),
        }));
        progress.set_stage(TaskStage::Finalizing);
        Ok(())
    }

    async fn cleanup_stale_temp(&self) {
        let settings = self.inner.settings.read().await.clone();
        let base = std::fs::canonicalize(&settings.mega_temp_dir)
            .unwrap_or_else(|_| PathBuf::from(&settings.mega_temp_dir));
        if !base.exists() {
            return;
        }
        let retention = settings.mega_retention_secs.max(0) as u64;
        let entries = match std::fs::read_dir(&base) {
            Ok(entries) => entries,
            Err(err) => {
                warn!(error = %err, "failed to scan MEGA temp directory");
                return;
            }
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if Uuid::parse_str(&name).is_err() {
                continue;
            }
            if let Some(running) = self.inner.tokens.read().unwrap().get(&name) {
                if !running.token.is_cancelled() {
                    continue;
                }
            }
            if let Ok(Some(record)) = self.inner.db.get_task(&name).await {
                if record.status_enum().is_active() {
                    continue;
                }
            }
            let age = entry
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(|modified| SystemTime::now().duration_since(modified).ok())
                .map(|duration| duration.as_secs())
                .unwrap_or(u64::MAX);
            if age < retention {
                continue;
            }
            match std::fs::remove_dir_all(&path) {
                Ok(()) => {
                    info!(task_id = %name, "removed stale MEGA temporary directory");
                    if let Ok(Some(record)) = self.inner.db.get_task(&name).await {
                        if record.temp_dir.is_some() {
                            if let Some(progress) = self.inner.progress.get(&name) {
                                progress.clear_temp();
                            }
                            let _ = self.inner.db.clear_task_temp(&name).await;
                        }
                    }
                }
                Err(err) => {
                    warn!(task_id = %name, error = %err, "failed to remove stale MEGA temp directory")
                }
            }
        }
    }

    async fn cleanup_orphan_multipart(&self) {
        let settings = self.inner.settings.read().await.clone();
        let tasks = match self.inner.db.list_tasks(1000).await {
            Ok(tasks) => tasks,
            Err(err) => {
                warn!(error = %err, "could not scan tasks for orphan multipart uploads");
                return;
            }
        };
        for task in tasks {
            let Some(upload_id) = task.s3_upload_id.as_deref() else {
                continue;
            };
            let Ok(Some(config)) = self.inner.db.get_s3_config(&task.s3_config_id).await else {
                continue;
            };
            match self
                .inner
                .s3
                .abort(
                    &config,
                    &task.bucket,
                    task.s3_key.as_deref().unwrap_or_default(),
                    upload_id,
                    settings.max_retries as u32,
                )
                .await
            {
                Ok(()) => {
                    let _ = self.inner.db.update_task_upload(&task.id, None, None).await;
                }
                Err(err) => {
                    warn!(task_id = %task.id, error = %err, "could not abort orphan multipart upload")
                }
            }
        }
    }

    async fn resolve_s3_config(&self, requested: Option<&str>) -> Result<S3ConfigRecord, AppError> {
        match requested.map(str::trim).filter(|v| !v.is_empty()) {
            Some(id) => self
                .inner
                .db
                .get_s3_config(id)
                .await?
                .ok_or_else(|| AppError::bad_request("Selected S3 configuration was not found")),
            None => self.inner.db.get_default_s3_config().await?.ok_or_else(|| {
                AppError::bad_request("No S3 configuration exists. Add one in Settings first.")
            }),
        }
    }
}

fn normalize_input_key(input: Option<&str>) -> Result<Option<String>, AppError> {
    let Some(input) = input.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    let trailing = input.ends_with('/');
    let normalized = normalize_object_key(input).map_err(AppError::bad_request)?;
    Ok(Some(if trailing {
        format!("{normalized}/")
    } else {
        normalized
    }))
}

fn validate_bucket(bucket: &str) -> Result<(), AppError> {
    if bucket.is_empty() || bucket.len() > 255 {
        return Err(AppError::bad_request("Bucket name is required"));
    }
    if bucket
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || c == '/' || c == '\\')
    {
        return Err(AppError::bad_request(
            "Bucket name contains invalid characters",
        ));
    }
    Ok(())
}

fn validate_headers(
    headers: &HashMap<String, String>,
) -> Result<HashMap<String, String>, AppError> {
    if headers.len() > 40 {
        return Err(AppError::bad_request("Too many custom headers"));
    }
    let mut out = HashMap::new();
    let mut total = 0usize;
    for (name, value) in headers {
        let lower = name.trim().to_ascii_lowercase();
        let header = HeaderName::from_bytes(lower.as_bytes())
            .map_err(|_| AppError::bad_request(format!("Invalid header name: {name}")))?;
        if matches!(
            header.as_str(),
            "host"
                | "content-length"
                | "transfer-encoding"
                | "connection"
                | "range"
                | "accept-encoding"
                | "upgrade"
                | "te"
                | "trailer"
        ) {
            return Err(AppError::bad_request(format!(
                "Header {name} is managed by the downloader and cannot be overridden"
            )));
        }
        if value.len() > 8192 {
            return Err(AppError::bad_request(format!("Header {name} is too long")));
        }
        HeaderValue::from_str(value)
            .map_err(|_| AppError::bad_request(format!("Invalid value for header {name}")))?;
        total += name.len() + value.len();
        out.insert(header.as_str().to_string(), value.clone());
    }
    if total > 32 * 1024 {
        return Err(AppError::bad_request("Custom headers are too large"));
    }
    Ok(out)
}

fn single_object_key(
    config: &S3ConfigRecord,
    record: &TaskRecord,
    filename: &str,
) -> Result<String, String> {
    let filename = sanitize_filename(filename);
    if let Some(raw) = record.object_key.as_deref() {
        if raw.ends_with('/') {
            let base = normalize_object_key(raw)?;
            return Ok(format!("{base}/{filename}"));
        }
        return normalize_object_key(raw);
    }
    let prefix = optional_prefix(&config.default_prefix)?;
    Ok(match prefix {
        Some(prefix) => format!("{prefix}/{filename}"),
        None => filename,
    })
}

fn mega_object_key(
    config: &S3ConfigRecord,
    record: &TaskRecord,
    kind: &str,
    file: &MegaFile,
) -> Result<String, String> {
    let relative = file.safe_relative_path.to_string_lossy().replace('\\', "/");
    if kind == "folder" {
        let prefix = match record.object_key.as_deref() {
            Some(raw) => Some(normalize_object_key(raw)?),
            None => optional_prefix(&config.default_prefix)?,
        };
        let key = match prefix {
            Some(prefix) => format!("{prefix}/{relative}"),
            None => relative,
        };
        return check_key_length(key);
    }
    if let Some(raw) = record.object_key.as_deref() {
        if raw.ends_with('/') {
            let base = normalize_object_key(raw)?;
            return check_key_length(format!("{base}/{relative}"));
        }
        return normalize_object_key(raw);
    }
    let prefix = optional_prefix(&config.default_prefix)?;
    Ok(match prefix {
        Some(prefix) => format!("{prefix}/{relative}"),
        None => relative,
    })
}

fn mega_folder_prefix(
    config: &S3ConfigRecord,
    record: &TaskRecord,
    files: &[MegaFile],
) -> Option<String> {
    if let Some(raw) = record.object_key.as_deref() {
        return normalize_object_key(raw).ok();
    }
    if let Ok(Some(prefix)) = optional_prefix(&config.default_prefix) {
        return Some(prefix);
    }
    files
        .first()
        .and_then(|file| file.safe_relative_path.components().next())
        .map(|component| component.as_os_str().to_string_lossy().to_string())
}

fn optional_prefix(raw: &str) -> Result<Option<String>, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    Ok(Some(normalize_object_key(raw)?))
}

fn check_key_length(key: String) -> Result<String, String> {
    if key.len() > 1024 {
        Err("Generated object key is too long".into())
    } else {
        Ok(key)
    }
}

fn truncate_error(message: &str, max: usize) -> String {
    if message.chars().count() <= max {
        message.to_string()
    } else {
        format!("{}...", message.chars().take(max).collect::<String>())
    }
}

async fn stream_local_file(
    path: &PathBuf,
    expected_size: i64,
    part_size: usize,
    tx: mpsc::Sender<StreamPart>,
    cancel: CancellationToken,
) -> Result<(), String> {
    let mut file = File::open(path)
        .await
        .map_err(|e| format!("cannot open downloaded file {}: {e}", path.display()))?;
    let mut part = BytesMut::with_capacity(part_size);
    let mut number = 1i32;
    let mut total = 0i64;
    loop {
        if cancel.is_cancelled() {
            return Err("Upload cancelled".into());
        }
        let read = tokio::select! {
            _ = cancel.cancelled() => return Err("Upload cancelled".into()),
            result = file.read_buf(&mut part) => result.map_err(|e| format!("file read failed: {e}"))?,
        };
        if read == 0 {
            break;
        }
        total += read as i64;
        while part.len() >= part_size {
            let data = part.split_to(part_size).freeze();
            send_file_part(&tx, number, data, &cancel).await?;
            number += 1;
        }
    }
    if !part.is_empty() {
        let data = part.split().freeze();
        send_file_part(&tx, number, data, &cancel).await?;
    }
    if total != expected_size {
        let message = format!(
            "Downloaded file size changed during upload: expected {expected_size}, read {total}"
        );
        let _ = tx
            .send(StreamPart::Failed {
                message: message.clone(),
            })
            .await;
        return Err(message);
    }
    let _ = tx.send(StreamPart::Finish { total_bytes: total }).await;
    Ok(())
}

async fn send_file_part(
    tx: &mpsc::Sender<StreamPart>,
    number: i32,
    data: bytes::Bytes,
    cancel: &CancellationToken,
) -> Result<(), String> {
    tokio::select! {
        _ = cancel.cancelled() => Err("Upload cancelled".into()),
        result = tx.send(StreamPart::Part { number, data }) => {
            result.map_err(|_| "Upload stream closed".to_string())
        }
    }
}
