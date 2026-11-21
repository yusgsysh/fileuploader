use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Task enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    #[default]
    Auto,
    Http,
    Mega,
}

impl SourceType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Http => "http",
            Self::Mega => "mega",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "http" => Self::Http,
            "mega" => Self::Mega,
            _ => Self::Auto,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolvedSource {
    Http,
    Mega,
}

impl ResolvedSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Mega => "mega",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "http" => Some(Self::Http),
            "mega" => Some(Self::Mega),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    #[default]
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "running" => Self::Running,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            _ => Self::Pending,
        }
    }

    pub fn is_active(self) -> bool {
        matches!(self, Self::Pending | Self::Running)
    }

    pub fn is_terminal(self) -> bool {
        !self.is_active()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskStage {
    #[default]
    Pending,
    Downloading,
    PreparingUpload,
    Uploading,
    Finalizing,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Downloading => "downloading",
            Self::PreparingUpload => "preparing_upload",
            Self::Uploading => "uploading",
            Self::Finalizing => "finalizing",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "downloading" => Self::Downloading,
            "preparing_upload" => Self::PreparingUpload,
            "uploading" => Self::Uploading,
            "finalizing" => Self::Finalizing,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            _ => Self::Pending,
        }
    }
}

// ---------------------------------------------------------------------------
// Authentication
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AuthAccount {
    pub username: String,
    pub password_hash: String,
    pub salt: String,
    pub iterations: i64,
    pub created_at: String,
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    // Multipart / transfer
    pub part_size_bytes: i64,
    pub upload_concurrency: i64,
    pub http_buffer_bytes: i64,
    pub max_pending_parts: i64,
    pub max_http_tasks: i64,
    pub max_mega_tasks: i64,
    pub max_queued_tasks: i64,
    pub max_retries: i64,
    pub http_timeout_secs: i64,
    pub max_redirects: i64,
    pub max_file_bytes: i64,
    pub task_timeout_secs: i64,

    // MEGA
    pub mega_get_path: String,
    pub mega_server_path: String,
    pub mega_whoami_path: String,
    pub mega_home: String,
    pub mega_temp_dir: String,
    pub mega_max_task_bytes: i64,
    pub mega_temp_max_bytes: i64,
    pub mega_retention_secs: i64,
    pub mega_max_downloads: i64,
    pub mega_auto_start_server: bool,
    pub mega_download_timeout_secs: i64,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            part_size_bytes: 16 * 1024 * 1024,
            upload_concurrency: 2,
            http_buffer_bytes: 1024 * 1024,
            max_pending_parts: 2,
            max_http_tasks: 3,
            max_mega_tasks: 1,
            max_queued_tasks: 50,
            max_retries: 3,
            http_timeout_secs: 30,
            max_redirects: 10,
            max_file_bytes: 100 * 1024 * 1024 * 1024,
            task_timeout_secs: 24 * 60 * 60,

            mega_get_path: "mega-get".into(),
            mega_server_path: "mega-cmd-server".into(),
            mega_whoami_path: "mega-whoami".into(),
            mega_home: "./data/mega-home".into(),
            mega_temp_dir: "./tmp/mega".into(),
            mega_max_task_bytes: 50 * 1024 * 1024 * 1024,
            mega_temp_max_bytes: 200 * 1024 * 1024 * 1024,
            mega_retention_secs: 24 * 60 * 60,
            mega_max_downloads: 1,
            mega_auto_start_server: true,
            mega_download_timeout_secs: 6 * 60 * 60,
        }
    }
}

impl AppSettings {
    pub fn with_env_defaults() -> Self {
        let mut s = Self::default();
        macro_rules! env_str {
            ($field:ident, $name:literal) => {
                if let Ok(v) = std::env::var($name) {
                    if !v.is_empty() {
                        s.$field = v;
                    }
                }
            };
        }
        macro_rules! env_i64 {
            ($field:ident, $name:literal) => {
                if let Ok(v) = std::env::var($name) {
                    if let Ok(v) = v.parse::<i64>() {
                        s.$field = v;
                    }
                }
            };
        }
        env_i64!(part_size_bytes, "PART_SIZE_BYTES");
        env_i64!(upload_concurrency, "UPLOAD_CONCURRENCY");
        env_i64!(http_buffer_bytes, "HTTP_BUFFER_BYTES");
        env_i64!(max_pending_parts, "MAX_PENDING_PARTS");
        env_i64!(max_http_tasks, "MAX_HTTP_TASKS");
        env_i64!(max_mega_tasks, "MAX_MEGA_TASKS");
        env_i64!(max_queued_tasks, "MAX_QUEUED_TASKS");
        env_i64!(max_retries, "MAX_RETRIES");
        env_i64!(http_timeout_secs, "HTTP_TIMEOUT_SECS");
        env_i64!(max_redirects, "MAX_REDIRECTS");
        env_i64!(max_file_bytes, "MAX_FILE_BYTES");
        env_i64!(task_timeout_secs, "TASK_TIMEOUT_SECS");
        env_str!(mega_get_path, "MEGA_GET_PATH");
        env_str!(mega_server_path, "MEGA_SERVER_PATH");
        env_str!(mega_whoami_path, "MEGA_WHOAMI_PATH");
        env_str!(mega_home, "MEGA_HOME");
        env_str!(mega_temp_dir, "MEGA_TEMP_DIR");
        env_i64!(mega_max_task_bytes, "MEGA_MAX_TASK_BYTES");
        env_i64!(mega_temp_max_bytes, "MEGA_TEMP_MAX_BYTES");
        env_i64!(mega_retention_secs, "MEGA_RETENTION_SECS");
        env_i64!(mega_max_downloads, "MEGA_MAX_DOWNLOADS");
        env_i64!(mega_download_timeout_secs, "MEGA_DOWNLOAD_TIMEOUT_SECS");
        if let Ok(v) = std::env::var("MEGA_AUTO_START_SERVER") {
            s.mega_auto_start_server = !matches!(v.as_str(), "0" | "false" | "no");
        }
        s
    }

    /// MEGAcmd interprets HOME relative to its working directory. Persist and
    /// use absolute paths so task directories and transfer tags always match.
    pub fn normalize_paths(&mut self) {
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let make_absolute = |value: &str| -> String {
            let path = std::path::PathBuf::from(value);
            if path.is_absolute() {
                path
            } else {
                cwd.join(path)
            }
            .to_string_lossy()
            .to_string()
        };
        self.mega_home = make_absolute(&self.mega_home);
        self.mega_temp_dir = make_absolute(&self.mega_temp_dir);
    }

    pub fn validate(&self) -> Result<(), String> {
        if !(5 * 1024 * 1024..=5 * 1024 * 1024 * 1024).contains(&self.part_size_bytes) {
            return Err("Part size must be between 5 MiB and 5 GiB".into());
        }
        if !(1..=16).contains(&self.upload_concurrency) {
            return Err("Upload concurrency must be between 1 and 16".into());
        }
        if !(64 * 1024..=16 * 1024 * 1024).contains(&self.http_buffer_bytes) {
            return Err("HTTP buffer must be between 64 KiB and 16 MiB".into());
        }
        if !(1..=32).contains(&self.max_pending_parts) {
            return Err("Maximum pending parts must be between 1 and 32".into());
        }
        if !(1..=32).contains(&self.max_http_tasks) {
            return Err("HTTP concurrency must be between 1 and 32".into());
        }
        if !(1..=16).contains(&self.max_mega_tasks) {
            return Err("MEGA concurrency must be between 1 and 16".into());
        }
        if !(1..=1000).contains(&self.max_queued_tasks) {
            return Err("Queued task limit must be between 1 and 1000".into());
        }
        if !(0..=10).contains(&self.max_retries) {
            return Err("Retries must be between 0 and 10".into());
        }
        if !(1..=3600).contains(&self.http_timeout_secs) {
            return Err("HTTP timeout must be between 1 and 3600 seconds".into());
        }
        if !(0..=20).contains(&self.max_redirects) {
            return Err("Maximum redirects must be between 0 and 20".into());
        }
        if self.max_file_bytes < 0 {
            return Err("Maximum file size cannot be negative (0 means unlimited)".into());
        }
        if !(60..=30 * 24 * 60 * 60).contains(&self.task_timeout_secs) {
            return Err("Task timeout must be between 60 seconds and 30 days".into());
        }
        if self.mega_get_path.trim().is_empty() || self.mega_server_path.trim().is_empty() {
            return Err("MEGA executable paths cannot be empty".into());
        }
        if self.mega_home.trim().is_empty() || self.mega_temp_dir.trim().is_empty() {
            return Err("MEGA home and temporary directory cannot be empty".into());
        }
        if self.mega_max_task_bytes < 0 || self.mega_temp_max_bytes < 0 {
            return Err("MEGA disk limits cannot be negative (0 means unlimited)".into());
        }
        if !(0..=90 * 24 * 60 * 60).contains(&self.mega_retention_secs) {
            return Err("MEGA retention must be between 0 and 90 days".into());
        }
        if !(1..=16).contains(&self.mega_max_downloads) {
            return Err("MEGA download concurrency must be between 1 and 16".into());
        }
        if !(60..=7 * 24 * 60 * 60).contains(&self.mega_download_timeout_secs) {
            return Err("MEGA download timeout must be between 1 minute and 7 days".into());
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// S3 configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct S3ConfigRecord {
    pub id: String,
    pub name: String,
    pub endpoint: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
    pub default_prefix: String,
    pub path_style: bool,
    pub use_tls: bool,
    pub metadata_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct S3ConfigDto {
    pub id: String,
    pub name: String,
    pub endpoint: String,
    pub region: String,
    pub access_key: String,
    pub bucket: String,
    pub default_prefix: String,
    pub path_style: bool,
    pub use_tls: bool,
    pub metadata: HashMap<String, String>,
    pub secret_set: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<&S3ConfigRecord> for S3ConfigDto {
    fn from(value: &S3ConfigRecord) -> Self {
        Self {
            id: value.id.clone(),
            name: value.name.clone(),
            endpoint: value.endpoint.clone(),
            region: value.region.clone(),
            access_key: value.access_key.clone(),
            bucket: value.bucket.clone(),
            default_prefix: value.default_prefix.clone(),
            path_style: value.path_style,
            use_tls: value.use_tls,
            metadata: parse_metadata(&value.metadata_json),
            secret_set: !value.secret_key.is_empty(),
            created_at: value.created_at.clone(),
            updated_at: value.updated_at.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct S3ConfigInput {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default = "default_region")]
    pub region: String,
    #[serde(default)]
    pub access_key: String,
    #[serde(default)]
    pub secret_key: Option<String>,
    #[serde(default)]
    pub bucket: String,
    #[serde(default)]
    pub default_prefix: String,
    #[serde(default)]
    pub path_style: bool,
    #[serde(default = "default_true")]
    pub use_tls: bool,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

fn default_region() -> String {
    "us-east-1".into()
}

fn default_true() -> bool {
    true
}

pub fn parse_metadata(raw: &str) -> HashMap<String, String> {
    serde_json::from_str(raw).unwrap_or_default()
}

#[derive(Debug, Clone, Serialize)]
pub struct BucketInfo {
    pub name: String,
    pub creation_date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct S3TestResult {
    pub ok: bool,
    pub message: String,
    pub bucket_count: Option<usize>,
    pub latency_ms: u128,
}

// ---------------------------------------------------------------------------
// Tasks
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct CreateTaskRequest {
    pub url: String,
    #[serde(default)]
    pub source_type: SourceType,
    #[serde(default)]
    pub s3_config_id: Option<String>,
    #[serde(default)]
    pub bucket: Option<String>,
    #[serde(default)]
    pub object_key: Option<String>,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
    #[serde(default)]
    pub referer: Option<String>,
    #[serde(default)]
    pub cookie: Option<String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub auth_username: Option<String>,
    #[serde(default)]
    pub auth_password: Option<String>,
    #[serde(default)]
    pub allow_overwrite: bool,
}

#[derive(Debug, Clone)]
pub struct TaskRecord {
    pub id: String,
    pub url: String,
    pub source_type: String,
    pub resolved_source: Option<String>,
    pub s3_config_id: String,
    pub bucket: String,
    pub object_key: Option<String>,
    pub filename: Option<String>,
    pub resolved_filename: Option<String>,
    pub content_type: Option<String>,
    pub total_bytes: Option<i64>,
    pub downloaded_bytes: i64,
    pub uploaded_bytes: i64,
    pub status: String,
    pub stage: String,
    pub error: Option<String>,
    pub s3_upload_id: Option<String>,
    pub s3_key: Option<String>,
    pub temp_dir: Option<String>,
    pub temp_complete: bool,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
    pub cookie: Option<String>,
    pub headers_json: String,
    pub auth_username: Option<String>,
    pub auth_password: Option<String>,
    pub allow_overwrite: bool,
    pub attempt: i64,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub range_supported: Option<bool>,
    pub temp_bytes: i64,
}

impl TaskRecord {
    pub fn status_enum(&self) -> TaskStatus {
        TaskStatus::parse(&self.status)
    }

    pub fn stage_enum(&self) -> TaskStage {
        TaskStage::parse(&self.stage)
    }

    pub fn resolved(&self) -> Option<ResolvedSource> {
        self.resolved_source
            .as_deref()
            .and_then(ResolvedSource::parse)
    }

    pub fn headers(&self) -> HashMap<String, String> {
        serde_json::from_str(&self.headers_json).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskDto {
    pub id: String,
    pub url: String,
    pub source_type: SourceType,
    pub resolved_source: Option<ResolvedSource>,
    pub s3_config_id: String,
    pub bucket: String,
    pub object_key: Option<String>,
    pub filename: String,
    pub content_type: Option<String>,
    pub total_bytes: Option<i64>,
    pub downloaded_bytes: i64,
    pub uploaded_bytes: i64,
    pub status: TaskStatus,
    pub stage: TaskStage,
    pub error: Option<String>,
    pub s3_upload_id: Option<String>,
    pub s3_key: Option<String>,
    pub temp_bytes: i64,
    pub temp_complete: bool,
    pub allow_overwrite: bool,
    pub attempt: i64,
    pub range_supported: Option<bool>,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub has_custom_headers: bool,
    pub auth_username: Option<String>,
    pub progress: ProgressDto,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ProgressDto {
    pub download_speed_bps: f64,
    pub upload_speed_bps: f64,
    pub download_progress: Option<f64>,
    pub upload_progress: Option<f64>,
    pub overall_progress: Option<f64>,
    pub eta_secs: Option<i64>,
    pub parts_completed: usize,
    pub parts_total: Option<usize>,
    pub parts: Vec<PartInfo>,
    pub mega: Option<MegaProgress>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartInfo {
    pub part_number: i32,
    pub size: i64,
    pub etag: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MegaProgress {
    pub process_state: String,
    pub temp_bytes: i64,
    pub temp_dir: Option<String>,
    pub files_total: usize,
    pub files_done: usize,
    pub current_file: Option<String>,
    pub percentage: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskHistoryItem {
    pub id: i64,
    pub attempt: i64,
    pub status: String,
    pub stage: String,
    pub error: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskDetailDto {
    #[serde(flatten)]
    pub task: TaskDto,
    pub history: Vec<TaskHistoryItem>,
}

// ---------------------------------------------------------------------------
// System / MEGA status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct SystemStatus {
    pub version: String,
    pub uptime_secs: u64,
    pub database_ok: bool,
    pub tasks: TaskCounts,
    pub active_tasks: usize,
    pub queued_tasks: usize,
    pub mega_temp_bytes: i64,
    pub mega_temp_dirs: usize,
    pub settings: RuntimeSummary,
    pub mega: MegaStatus,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct TaskCounts {
    pub total: i64,
    pub pending: i64,
    pub running: i64,
    pub completed: i64,
    pub failed: i64,
    pub cancelled: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeSummary {
    pub max_http_tasks: i64,
    pub max_mega_tasks: i64,
    pub upload_concurrency: i64,
    pub part_size_bytes: i64,
    pub http_buffer_bytes: i64,
    pub max_pending_parts: i64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct MegaStatus {
    pub installed: bool,
    pub server_running: bool,
    pub logged_in: bool,
    pub account: Option<String>,
    pub version: Option<String>,
    pub get_path: String,
    pub server_path: String,
    pub home: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct MegaTestRequest {
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MegaTestResult {
    pub ok: bool,
    pub checks: Vec<MegaCheck>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MegaCheck {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

// ---------------------------------------------------------------------------
// SSE
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerEvent {
    Task { task: Box<TaskDto> },
    TaskDeleted { task_id: String },
    Hello { time: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_defaults_are_valid() {
        AppSettings::default().validate().unwrap();
    }

    #[test]
    fn status_roundtrip() {
        for status in [
            TaskStatus::Pending,
            TaskStatus::Running,
            TaskStatus::Completed,
            TaskStatus::Failed,
            TaskStatus::Cancelled,
        ] {
            assert_eq!(TaskStatus::parse(status.as_str()), status);
        }
    }
}
