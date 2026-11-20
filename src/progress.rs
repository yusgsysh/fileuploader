use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, RwLock,
    },
    time::Instant,
};

use chrono::Utc;
use tokio::sync::{broadcast, mpsc as async_mpsc, oneshot};
use tracing::warn;

use crate::{
    db::{now, Store},
    models::{
        MegaProgress, PartInfo, ProgressDto, ResolvedSource, ServerEvent, SourceType, TaskDto,
        TaskRecord, TaskStage, TaskStatus,
    },
};

pub struct ProgressState {
    pub task: RwLock<TaskRecord>,
    pub downloaded: AtomicU64,
    pub uploaded: AtomicU64,
    pub parts: RwLock<Vec<PartInfo>>,
    pub parts_total: RwLock<Option<usize>>,
    pub mega: RwLock<Option<MegaProgress>>,
    pub speed: RwLock<SpeedState>,
    pub started: Instant,
    pub persist_counter: AtomicU64,
    db_tx: async_mpsc::UnboundedSender<DbCommand>,
}

enum DbCommand {
    Status {
        id: String,
        status: TaskStatus,
        stage: TaskStage,
        error: Option<String>,
        finished: bool,
        ack: Option<oneshot::Sender<()>>,
    },
    Source {
        id: String,
        resolved_source: String,
        filename: Option<String>,
        content_type: Option<String>,
        total: Option<i64>,
        range_supported: Option<bool>,
    },
    Upload {
        id: String,
        upload_id: Option<String>,
        key: Option<String>,
    },
    Temp {
        id: String,
        dir: Option<String>,
        complete: bool,
        bytes: i64,
    },
    ClearTemp {
        id: String,
    },
    Counters {
        id: String,
        downloaded: i64,
        uploaded: i64,
        total: Option<i64>,
        temp: i64,
    },
}

fn spawn_db_writer(db: Store, mut rx: async_mpsc::UnboundedReceiver<DbCommand>) {
    tokio::spawn(async move {
        while let Some(command) = rx.recv().await {
            match command {
                DbCommand::Status {
                    id,
                    status,
                    stage,
                    error,
                    finished,
                    ack,
                } => {
                    if let Err(err) = db
                        .update_task_status(&id, status, stage, error.as_deref(), finished)
                        .await
                    {
                        warn!(task_id = %id, error = %err, "failed to persist task status");
                    }
                    if let Some(ack) = ack {
                        let _ = ack.send(());
                    }
                }
                DbCommand::Source {
                    id,
                    resolved_source,
                    filename,
                    content_type,
                    total,
                    range_supported,
                } => {
                    if let Err(err) = db
                        .update_task_source(
                            &id,
                            &resolved_source,
                            filename.as_deref(),
                            content_type.as_deref(),
                            total,
                            range_supported,
                        )
                        .await
                    {
                        warn!(task_id = %id, error = %err, "failed to persist task source");
                    }
                }
                DbCommand::Upload { id, upload_id, key } => {
                    if let Err(err) = db
                        .update_task_upload(&id, upload_id.as_deref(), key.as_deref())
                        .await
                    {
                        warn!(task_id = %id, error = %err, "failed to persist upload state");
                    }
                }
                DbCommand::Temp {
                    id,
                    dir,
                    complete,
                    bytes,
                } => {
                    if let Err(err) = db
                        .update_task_temp(&id, dir.as_deref(), complete, bytes)
                        .await
                    {
                        warn!(task_id = %id, error = %err, "failed to persist temp state");
                    }
                }
                DbCommand::ClearTemp { id } => {
                    if let Err(err) = db.clear_task_temp(&id).await {
                        warn!(task_id = %id, error = %err, "failed to clear temp state");
                    }
                }
                DbCommand::Counters {
                    id,
                    downloaded,
                    uploaded,
                    total,
                    temp,
                } => {
                    if let Err(err) = db
                        .update_task_counters(&id, downloaded, uploaded, total, temp)
                        .await
                    {
                        warn!(task_id = %id, error = %err, "failed to persist task progress");
                    }
                }
            }
        }
    });
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SpeedState {
    pub last_at: Option<Instant>,
    pub last_downloaded: u64,
    pub last_uploaded: u64,
    pub download_bps: f64,
    pub upload_bps: f64,
}

#[derive(Clone)]
pub struct ProgressHandle {
    pub inner: Arc<ProgressState>,
    events: broadcast::Sender<ServerEvent>,
}

#[derive(Clone)]
pub struct ProgressRegistry {
    states: Arc<RwLock<HashMap<String, Arc<ProgressState>>>>,
    db: Store,
    events: broadcast::Sender<ServerEvent>,
}

impl ProgressRegistry {
    pub fn new(db: Store, events: broadcast::Sender<ServerEvent>) -> Self {
        Self {
            states: Arc::new(RwLock::new(HashMap::new())),
            db,
            events,
        }
    }

    pub fn create(&self, record: TaskRecord) -> ProgressHandle {
        let downloaded = record.downloaded_bytes.max(0) as u64;
        let uploaded = record.uploaded_bytes.max(0) as u64;
        let (db_tx, db_rx) = async_mpsc::unbounded_channel();
        spawn_db_writer(self.db.clone(), db_rx);
        let state = Arc::new(ProgressState {
            task: RwLock::new(record.clone()),
            downloaded: AtomicU64::new(downloaded),
            uploaded: AtomicU64::new(uploaded),
            parts: RwLock::new(Vec::new()),
            parts_total: RwLock::new(None),
            mega: RwLock::new(None),
            speed: RwLock::new(SpeedState::default()),
            started: Instant::now(),
            persist_counter: AtomicU64::new(0),
            db_tx,
        });
        self.states
            .write()
            .unwrap()
            .insert(record.id.clone(), state.clone());
        let handle = ProgressHandle {
            inner: state,
            events: self.events.clone(),
        };
        handle.emit_now();
        handle
    }

    pub fn get(&self, id: &str) -> Option<ProgressHandle> {
        self.states
            .read()
            .unwrap()
            .get(id)
            .map(|state| ProgressHandle {
                inner: state.clone(),
                events: self.events.clone(),
            })
    }

    pub fn remove(&self, id: &str) {
        self.states.write().unwrap().remove(id);
    }

    pub fn active_count(&self) -> usize {
        self.states
            .read()
            .unwrap()
            .values()
            .filter(|state| {
                state
                    .task
                    .read()
                    .map(|t| t.status_enum().is_active())
                    .unwrap_or(false)
            })
            .count()
    }

    pub fn dto_or_record(&self, record: &TaskRecord) -> TaskDto {
        self.dto_or_record_with_parts(record, true)
    }

    pub fn summary_or_record(&self, record: &TaskRecord) -> TaskDto {
        self.dto_or_record_with_parts(record, false)
    }

    fn dto_or_record_with_parts(&self, record: &TaskRecord, include_parts: bool) -> TaskDto {
        match self.get(&record.id) {
            Some(handle) => handle.dto_with_parts(include_parts),
            None => record_to_dto(record, &ProgressDto::default(), 0, 0),
        }
    }

    pub fn tick(&self) {
        let states: Vec<(String, Arc<ProgressState>)> = self
            .states
            .read()
            .unwrap()
            .iter()
            .map(|(id, state)| (id.clone(), state.clone()))
            .collect();
        for (id, state) in states {
            let status = state
                .task
                .read()
                .map(|t| t.status_enum())
                .unwrap_or(TaskStatus::Failed);
            let downloaded = state.downloaded.load(Ordering::Relaxed);
            let uploaded = state.uploaded.load(Ordering::Relaxed);
            let now = Instant::now();
            {
                let mut speed = state.speed.write().unwrap();
                if let Some(last_at) = speed.last_at {
                    let secs = now.duration_since(last_at).as_secs_f64().max(0.001);
                    let instant_down =
                        downloaded.saturating_sub(speed.last_downloaded) as f64 / secs;
                    let instant_up = uploaded.saturating_sub(speed.last_uploaded) as f64 / secs;
                    let alpha = 0.35;
                    speed.download_bps = if speed.download_bps == 0.0 {
                        instant_down
                    } else {
                        speed.download_bps * (1.0 - alpha) + instant_down * alpha
                    };
                    speed.upload_bps = if speed.upload_bps == 0.0 {
                        instant_up
                    } else {
                        speed.upload_bps * (1.0 - alpha) + instant_up * alpha
                    };
                    if instant_down == 0.0 && speed.download_bps < 2.0 {
                        speed.download_bps = 0.0;
                    }
                    if instant_up == 0.0 && speed.upload_bps < 2.0 {
                        speed.upload_bps = 0.0;
                    }
                }
                speed.last_at = Some(now);
                speed.last_downloaded = downloaded;
                speed.last_uploaded = uploaded;
                if status.is_terminal() {
                    speed.download_bps = 0.0;
                    speed.upload_bps = 0.0;
                }
            }

            if status.is_active() {
                let handle = ProgressHandle {
                    inner: state.clone(),
                    events: self.events.clone(),
                };
                handle.emit_now();
                // Persist counters about once per second. SSE stays at 2 Hz.
                let tick = state.persist_counter.fetch_add(1, Ordering::Relaxed);
                if tick % 2 == 0 {
                    let task = state.task.read().unwrap().clone();
                    let _ = state.db_tx.send(DbCommand::Counters {
                        id: id.clone(),
                        downloaded: downloaded as i64,
                        uploaded: uploaded as i64,
                        total: task.total_bytes,
                        temp: task.temp_bytes,
                    });
                }
            }
        }
    }
}

impl ProgressHandle {
    pub fn id(&self) -> String {
        self.inner
            .task
            .read()
            .map(|t| t.id.clone())
            .unwrap_or_default()
    }

    pub fn record(&self) -> TaskRecord {
        self.inner.task.read().unwrap().clone()
    }

    pub fn add_downloaded(&self, bytes: u64) {
        self.inner.downloaded.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn set_downloaded(&self, bytes: u64) {
        self.inner.downloaded.store(bytes, Ordering::Relaxed);
    }

    pub fn add_uploaded(&self, bytes: u64) {
        self.inner.uploaded.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn set_uploaded(&self, bytes: u64) {
        self.inner.uploaded.store(bytes, Ordering::Relaxed);
    }

    pub fn downloaded(&self) -> u64 {
        self.inner.downloaded.load(Ordering::Relaxed)
    }

    pub fn uploaded(&self) -> u64 {
        self.inner.uploaded.load(Ordering::Relaxed)
    }

    pub fn set_stage(&self, stage: TaskStage) {
        {
            let mut task = self.inner.task.write().unwrap();
            task.stage = stage.as_str().to_string();
            task.updated_at = now();
        }
        self.persist_status();
    }

    pub fn set_status(&self, status: TaskStatus, stage: TaskStage, error: Option<String>) {
        {
            let mut task = self.inner.task.write().unwrap();
            task.status = status.as_str().to_string();
            task.stage = stage.as_str().to_string();
            task.error = error;
            task.updated_at = now();
            if status.is_terminal() {
                task.finished_at = Some(now());
            }
        }
        self.persist_status();
        self.emit_now();
    }

    pub fn set_error(&self, error: impl Into<String>) {
        let mut task = self.inner.task.write().unwrap();
        task.error = Some(error.into());
        task.updated_at = now();
    }

    pub fn set_source(
        &self,
        source: ResolvedSource,
        filename: Option<String>,
        content_type: Option<String>,
        total: Option<i64>,
        range_supported: Option<bool>,
    ) {
        {
            let mut task = self.inner.task.write().unwrap();
            task.resolved_source = Some(source.as_str().to_string());
            if let Some(filename) = filename {
                task.resolved_filename = Some(filename);
            }
            if content_type.is_some() {
                task.content_type = content_type;
            }
            if total.is_some() {
                task.total_bytes = total;
            }
            task.range_supported = range_supported;
            task.updated_at = now();
        }
        self.persist_meta();
    }

    pub fn set_upload(&self, upload_id: Option<String>, s3_key: Option<String>) {
        {
            let mut task = self.inner.task.write().unwrap();
            task.s3_upload_id = upload_id;
            if s3_key.is_some() {
                task.s3_key = s3_key;
            }
        }
        self.persist_upload();
    }

    pub fn set_s3_key(&self, key: Option<String>) {
        {
            let mut task = self.inner.task.write().unwrap();
            task.s3_key = key;
        }
        self.persist_upload();
    }

    pub fn set_temp(&self, temp_dir: Option<String>, temp_complete: bool, temp_bytes: i64) {
        {
            let mut task = self.inner.task.write().unwrap();
            if temp_dir.is_some() {
                task.temp_dir = temp_dir;
            }
            task.temp_complete = temp_complete;
            task.temp_bytes = temp_bytes;
        }
        self.persist_temp();
    }

    pub fn set_temp_bytes(&self, temp_bytes: i64) {
        let mut task = self.inner.task.write().unwrap();
        task.temp_bytes = temp_bytes;
    }

    pub fn clear_temp(&self) {
        {
            let mut task = self.inner.task.write().unwrap();
            task.temp_dir = None;
            task.temp_complete = false;
            task.temp_bytes = 0;
        }
        self.persist_clear_temp();
    }

    pub fn set_total(&self, total: Option<i64>) {
        {
            let mut task = self.inner.task.write().unwrap();
            task.total_bytes = total;
            task.updated_at = now();
        }
        self.persist_meta();
    }

    pub fn mark_started(&self) {
        {
            let mut task = self.inner.task.write().unwrap();
            task.status = TaskStatus::Running.as_str().to_string();
            if task.started_at.is_none() {
                task.started_at = Some(now());
            }
            task.updated_at = now();
        }
    }

    pub fn set_mega(&self, mega: Option<MegaProgress>) {
        *self.inner.mega.write().unwrap() = mega;
    }

    pub fn set_parts_total(&self, total: Option<usize>) {
        *self.inner.parts_total.write().unwrap() = total;
    }

    pub fn reset_parts(&self) {
        self.inner.parts.write().unwrap().clear();
        *self.inner.parts_total.write().unwrap() = None;
    }

    pub fn part_completed(&self, part: PartInfo) {
        let mut parts = self.inner.parts.write().unwrap();
        if let Some(existing) = parts.iter_mut().find(|p| p.part_number == part.part_number) {
            *existing = part;
        } else {
            parts.push(part);
            parts.sort_by_key(|p| p.part_number);
        }
    }

    pub fn emit_now(&self) {
        let _ = self.events.send(ServerEvent::Task {
            task: Box::new(self.dto()),
        });
    }

    pub fn dto(&self) -> TaskDto {
        self.dto_with_parts(true)
    }

    pub fn dto_with_parts(&self, include_parts: bool) -> TaskDto {
        let task = self.inner.task.read().unwrap().clone();
        let downloaded = self.inner.downloaded.load(Ordering::Relaxed);
        let uploaded = self.inner.uploaded.load(Ordering::Relaxed);
        let speed = *self.inner.speed.read().unwrap();
        let mut parts = self.inner.parts.read().unwrap().clone();
        // Keep SSE and detail payloads bounded even for 10,000-part uploads.
        if parts.len() > 100 {
            parts.drain(0..parts.len() - 100);
        }
        if !include_parts {
            parts.clear();
        }
        let parts_total = *self.inner.parts_total.read().unwrap();
        let mega = self.inner.mega.read().unwrap().clone();
        let total = task.total_bytes;
        let download_progress = percent(downloaded, total);
        let upload_progress = percent(uploaded, total);
        let overall_progress = match (download_progress, upload_progress) {
            (Some(d), Some(u)) => Some(((d + u) / 2.0).clamp(0.0, 100.0)),
            _ => None,
        };
        let eta_secs = estimate_eta(total, downloaded, uploaded, speed, task.stage_enum());
        let progress = ProgressDto {
            download_speed_bps: if task.status_enum().is_active() {
                speed.download_bps
            } else {
                0.0
            },
            upload_speed_bps: if task.status_enum().is_active() {
                speed.upload_bps
            } else {
                0.0
            },
            download_progress,
            upload_progress,
            overall_progress,
            eta_secs,
            parts_completed: self.inner.parts.read().unwrap().len(),
            parts_total,
            parts,
            mega,
        };
        record_to_dto(&task, &progress, downloaded, uploaded)
    }

    fn enqueue(&self, command: DbCommand) {
        let _ = self.inner.db_tx.send(command);
    }

    fn persist_status(&self) {
        let task = self.record();
        self.enqueue(DbCommand::Status {
            id: task.id.clone(),
            status: task.status_enum(),
            stage: task.stage_enum(),
            error: task.error.clone(),
            finished: task.status_enum().is_terminal(),
            ack: None,
        });
    }

    pub async fn set_status_awaited(
        &self,
        status: TaskStatus,
        stage: TaskStage,
        error: Option<String>,
    ) {
        {
            let mut task = self.inner.task.write().unwrap();
            task.status = status.as_str().to_string();
            task.stage = stage.as_str().to_string();
            task.error = error.clone();
            task.updated_at = now();
            if status.is_terminal() {
                task.finished_at = Some(now());
            }
        }
        let id = self.id();
        let (ack, wait) = oneshot::channel();
        self.enqueue(DbCommand::Status {
            id,
            status,
            stage,
            error,
            finished: status.is_terminal(),
            ack: Some(ack),
        });
        let _ = wait.await;
        self.emit_now();
    }

    pub fn persist_counters(&self) {
        let task = self.record();
        self.enqueue(DbCommand::Counters {
            id: task.id.clone(),
            downloaded: self.downloaded().min(i64::MAX as u64) as i64,
            uploaded: self.uploaded().min(i64::MAX as u64) as i64,
            total: task.total_bytes,
            temp: task.temp_bytes,
        });
    }

    fn persist_meta(&self) {
        let task = self.record();
        self.enqueue(DbCommand::Source {
            id: task.id.clone(),
            resolved_source: task
                .resolved_source
                .as_deref()
                .unwrap_or("http")
                .to_string(),
            filename: task.resolved_filename.clone(),
            content_type: task.content_type.clone(),
            total: task.total_bytes,
            range_supported: task.range_supported,
        });
    }

    fn persist_upload(&self) {
        let task = self.record();
        self.enqueue(DbCommand::Upload {
            id: task.id.clone(),
            upload_id: task.s3_upload_id.clone(),
            key: task.s3_key.clone(),
        });
    }

    fn persist_temp(&self) {
        let task = self.record();
        self.enqueue(DbCommand::Temp {
            id: task.id.clone(),
            dir: task.temp_dir.clone(),
            complete: task.temp_complete,
            bytes: task.temp_bytes,
        });
    }

    fn persist_clear_temp(&self) {
        self.enqueue(DbCommand::ClearTemp { id: self.id() });
    }
}

fn record_to_dto(
    task: &TaskRecord,
    progress: &ProgressDto,
    downloaded: u64,
    uploaded: u64,
) -> TaskDto {
    TaskDto {
        id: task.id.clone(),
        url: task.url.clone(),
        source_type: SourceType::parse(&task.source_type),
        resolved_source: task.resolved(),
        s3_config_id: task.s3_config_id.clone(),
        bucket: task.bucket.clone(),
        object_key: task.object_key.clone(),
        filename: task
            .resolved_filename
            .clone()
            .or_else(|| task.filename.clone())
            .unwrap_or_else(|| "download".into()),
        content_type: task.content_type.clone(),
        total_bytes: task.total_bytes,
        downloaded_bytes: downloaded.min(i64::MAX as u64) as i64,
        uploaded_bytes: uploaded.min(i64::MAX as u64) as i64,
        status: task.status_enum(),
        stage: task.stage_enum(),
        error: task.error.clone(),
        s3_upload_id: task.s3_upload_id.clone(),
        s3_key: task.s3_key.clone(),
        temp_bytes: task.temp_bytes,
        temp_complete: task.temp_complete,
        allow_overwrite: task.allow_overwrite,
        attempt: task.attempt,
        range_supported: task.range_supported,
        created_at: task.created_at.clone(),
        updated_at: task.updated_at.clone(),
        started_at: task.started_at.clone(),
        finished_at: task.finished_at.clone(),
        has_custom_headers: !task.headers().is_empty(),
        auth_username: task.auth_username.clone(),
        progress: progress.clone(),
    }
}

fn percent(current: u64, total: Option<i64>) -> Option<f64> {
    match total {
        Some(total) if total > 0 => {
            Some(((current as f64 / total as f64) * 100.0).clamp(0.0, 100.0))
        }
        _ => None,
    }
}

fn estimate_eta(
    total: Option<i64>,
    downloaded: u64,
    uploaded: u64,
    speed: SpeedState,
    stage: TaskStage,
) -> Option<i64> {
    let total = total? as f64;
    if total <= 0.0 {
        return None;
    }
    let downloaded = downloaded as f64;
    let uploaded = uploaded as f64;
    let down_rem = (total - downloaded).max(0.0);
    let up_rem = (total - uploaded).max(0.0);
    let down_speed = speed.download_bps.max(1.0);
    let up_speed = speed.upload_bps.max(1.0);
    let secs = match stage {
        TaskStage::Downloading | TaskStage::PreparingUpload => {
            if down_rem > 0.0 {
                down_rem / down_speed
            } else {
                up_rem / up_speed
            }
        }
        TaskStage::Uploading | TaskStage::Finalizing => {
            if up_rem > 0.0 {
                up_rem / up_speed
            } else {
                down_rem / down_speed
            }
        }
        _ => {
            // HTTP downloads and uploads overlap. Use the slower pipeline's
            // remaining time instead of adding both.
            (down_rem / down_speed).max(up_rem / up_speed)
        }
    };
    if secs.is_finite() && secs >= 0.0 {
        Some(secs.ceil().min(i64::MAX as f64) as i64)
    } else {
        None
    }
}

pub fn timestamp() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
