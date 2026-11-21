use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::{Duration, Instant},
};

use chrono::Utc;
use tokio::{
    io::AsyncReadExt,
    process::Command,
    sync::{mpsc, Mutex},
    time::{interval, sleep, timeout, MissedTickBehavior},
};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
use walkdir::WalkDir;

use crate::{
    models::{AppSettings, MegaCheck, MegaProgress, MegaStatus, MegaTestResult},
    progress::ProgressHandle,
    security::{sanitize_filename, validate_mega_share_url},
};

pub const COMPLETE_MARKER: &str = ".fileuploader-complete";

#[derive(Debug, thiserror::Error)]
pub enum MegaError {
    #[error("MEGA download cancelled")]
    Cancelled,
    #[error("MEGAcmd is not installed or not executable: {0}")]
    NotInstalled(String),
    #[error("MEGAcmd server is not running and auto-start is disabled")]
    ServerNotRunning,
    #[error("MEGAcmd server failed to start: {0}")]
    ServerStart(String),
    #[error("MEGAcmd process failed: {0}")]
    Process(String),
    #[error("MEGA download exceeded the per-task disk limit of {0} bytes")]
    TaskDiskLimit(i64),
    #[error("MEGA temporary storage exceeded the configured limit of {0} bytes")]
    GlobalDiskLimit(i64),
    #[error("MEGA download timed out")]
    Timeout,
    #[error("MEGA did not produce any downloadable files")]
    NoFiles,
    #[error("Unsafe MEGA download directory: {0}")]
    UnsafePath(String),
    #[error("MEGA download contains an unsupported symbolic link: {0}")]
    Symlink(String),
    #[error("MEGA filesystem error: {0}")]
    Io(String),
}

impl MegaError {
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }
}

#[derive(Debug, Clone)]
pub struct MegaFile {
    pub local_path: PathBuf,
    pub relative_path: PathBuf,
    pub safe_relative_path: PathBuf,
    pub size: i64,
    pub content_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MegaDownloadResult {
    pub files: Vec<MegaFile>,
    pub total_bytes: i64,
    pub temp_dir: PathBuf,
}

#[derive(Clone, Default)]
pub struct MegaService {
    start_lock: Arc<Mutex<()>>,
}

#[derive(Debug)]
struct WhoamiResult {
    server_running: bool,
    logged_in: bool,
    account: Option<String>,
}

impl MegaService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn task_dir(&self, settings: &AppSettings, task_id: &str) -> Result<PathBuf, MegaError> {
        if !task_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err(MegaError::UnsafePath("invalid task id".into()));
        }
        let base = PathBuf::from(&settings.mega_temp_dir);
        crate::config::create_private_dir(&base).map_err(|e| MegaError::Io(e.to_string()))?;
        Ok(base.join(task_id))
    }

    pub fn marker_path(task_dir: &Path) -> PathBuf {
        task_dir.join(COMPLETE_MARKER)
    }

    pub fn is_download_complete(task_dir: &Path) -> bool {
        task_dir.join(COMPLETE_MARKER).is_file()
    }

    /// Reuse a completed mega-get directory after a failed upload or a server
    /// restart. This avoids downloading the same public link again.
    pub fn load_completed(
        &self,
        settings: &AppSettings,
        task_id: &str,
        progress: &ProgressHandle,
    ) -> Result<Option<MegaDownloadResult>, MegaError> {
        let task_dir = self.task_dir(settings, task_id)?;
        if !Self::is_download_complete(&task_dir) || !dir_has_files(&task_dir) {
            return Ok(None);
        }
        let files = collect_files(&task_dir)?;
        let total = sum_sizes(&files)?;
        progress.set_temp(Some(path_string(&task_dir)), true, dir_size(&task_dir));
        progress.set_downloaded(total.max(0) as u64);
        progress.set_total(Some(total));
        progress.set_mega(Some(MegaProgress {
            process_state: "completed".into(),
            temp_bytes: dir_size(&task_dir),
            temp_dir: Some(path_string(&task_dir)),
            files_total: files.len(),
            files_done: files.len(),
            current_file: None,
            percentage: Some(100.0),
        }));
        Ok(Some(MegaDownloadResult {
            files,
            total_bytes: total,
            temp_dir: task_dir,
        }))
    }

    pub async fn status(&self, settings: &AppSettings) -> MegaStatus {
        let mut status = MegaStatus {
            get_path: settings.mega_get_path.clone(),
            server_path: settings.mega_server_path.clone(),
            home: settings.mega_home.clone(),
            ..Default::default()
        };
        match self.check_executable(&settings.mega_get_path) {
            Ok(()) => status.installed = true,
            Err(err) => {
                status.message = Some(err);
                return status;
            }
        }
        if let Ok(version) = self.version(settings).await {
            status.version = Some(version);
        }
        match self.whoami(settings).await {
            Ok(whoami) => {
                status.server_running = whoami.server_running;
                status.logged_in = whoami.logged_in;
                status.account = whoami.account;
                if !whoami.server_running {
                    status.message = Some(
                        "MEGAcmd server is not reachable. Enable auto-start or start it in the container."
                            .into(),
                    );
                }
            }
            Err(err) => {
                status.message = Some(err);
            }
        }
        status
    }

    pub async fn test(&self, settings: &AppSettings, url: Option<&str>) -> MegaTestResult {
        let mut checks = Vec::new();
        match self.check_executable(&settings.mega_get_path) {
            Ok(()) => checks.push(MegaCheck {
                name: "mega-get".into(),
                ok: true,
                detail: settings.mega_get_path.clone(),
            }),
            Err(err) => checks.push(MegaCheck {
                name: "mega-get".into(),
                ok: false,
                detail: err,
            }),
        }
        match self.check_executable(&settings.mega_server_path) {
            Ok(()) => checks.push(MegaCheck {
                name: "mega-cmd-server".into(),
                ok: true,
                detail: settings.mega_server_path.clone(),
            }),
            Err(err) => checks.push(MegaCheck {
                name: "mega-cmd-server".into(),
                ok: false,
                detail: err,
            }),
        }
        let status = self.status(settings).await;
        checks.push(MegaCheck {
            name: "MEGAcmd server".into(),
            ok: status.server_running,
            detail: if status.server_running {
                "reachable".into()
            } else {
                status
                    .message
                    .clone()
                    .unwrap_or_else(|| "not reachable".into())
            },
        });
        checks.push(MegaCheck {
            name: "Session".into(),
            ok: true,
            detail: if status.logged_in {
                format!(
                    "logged in as {} (public links also work without login)",
                    status.account.unwrap_or_else(|| "MEGA account".into())
                )
            } else {
                "not logged in; public share links can still be downloaded".into()
            },
        });
        if let Some(url) = url {
            let detail = match validate_mega_share_url(url) {
                Ok(parsed) => format!(
                    "valid {} share link; the key is preserved",
                    crate::security::mega_link_kind(&parsed)
                ),
                Err(err) => err,
            };
            checks.push(MegaCheck {
                name: "Share link".into(),
                ok: validate_mega_share_url(url).is_ok(),
                detail,
            });
        }
        MegaTestResult {
            ok: checks.iter().all(|check| check.ok),
            checks,
        }
    }

    pub async fn ensure_server(&self, settings: &AppSettings) -> Result<(), MegaError> {
        self.check_executable(&settings.mega_get_path)
            .map_err(MegaError::NotInstalled)?;
        let _guard = self.start_lock.lock().await;
        if let Ok(whoami) = self.whoami(settings).await {
            if whoami.server_running {
                return Ok(());
            }
        }
        if !settings.mega_auto_start_server {
            return Err(MegaError::ServerNotRunning);
        }
        self.start_server(settings).await
    }

    async fn start_server(&self, settings: &AppSettings) -> Result<(), MegaError> {
        crate::config::create_private_dir(Path::new(&settings.mega_home))
            .map_err(|e| MegaError::ServerStart(e.to_string()))?;
        let log_path = Path::new(&settings.mega_home).join("server.log");
        let stdout = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| MegaError::ServerStart(format!("cannot open server log: {e}")))?;
        let stderr = stdout
            .try_clone()
            .map_err(|e| MegaError::ServerStart(format!("cannot open server log: {e}")))?;
        let mut command = Command::new(&settings.mega_server_path);
        command
            .env("HOME", &settings.mega_home)
            .current_dir(&settings.mega_home)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .kill_on_drop(false);
        #[cfg(unix)]
        {
            command.process_group(0);
        }
        let child = command
            .spawn()
            .map_err(|e| MegaError::ServerStart(e.to_string()))?;
        let pid = child.id();
        // The server intentionally outlives this child handle. MEGAcmd's
        // server persists its session below $HOME/.megaCmd.
        drop(child);
        info!(
            home = %settings.mega_home,
            pid = ?pid,
            "starting MEGAcmd server"
        );
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            if let Ok(whoami) = self.whoami(settings).await {
                if whoami.server_running {
                    return Ok(());
                }
            }
            sleep(Duration::from_millis(400)).await;
        }
        Err(MegaError::ServerStart(
            "server did not become reachable within 20 seconds; inspect server.log".into(),
        ))
    }

    pub async fn download(
        &self,
        settings: &AppSettings,
        url: &str,
        password: Option<&str>,
        task_id: &str,
        progress: ProgressHandle,
        cancel: CancellationToken,
    ) -> Result<MegaDownloadResult, MegaError> {
        validate_mega_share_url(url)
            .map_err(|e| MegaError::Process(format!("invalid share link: {e}")))?;
        if let Some(result) = self.load_completed(settings, task_id, &progress)? {
            return Ok(result);
        }
        self.ensure_server(settings).await?;
        let task_dir = self.task_dir(settings, task_id)?;
        if task_dir.exists() {
            // A fresh attempt may reuse a completed directory, but a partial
            // directory from an earlier attempt must never be mixed with a new
            // mega-get run.
            std::fs::remove_dir_all(&task_dir)
                .map_err(|e| MegaError::Io(format!("cannot clear task directory: {e}")))?;
        }
        crate::config::create_private_dir(&task_dir).map_err(|e| MegaError::Io(e.to_string()))?;
        progress.set_temp(Some(path_string(&task_dir)), false, 0);

        enforce_global_limit(settings)?;
        let mut command = Command::new(&settings.mega_get_path);
        command.arg("--ignore-quota-warn");
        if let Some(password) = password.filter(|value| !value.is_empty()) {
            command.arg(format!("--password={password}"));
        }
        command
            .arg(url)
            .arg(&task_dir)
            .env("HOME", &settings.mega_home)
            .current_dir(&settings.mega_home)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        {
            command.process_group(0);
        }
        let mut child = command.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                MegaError::NotInstalled(settings.mega_get_path.clone())
            } else {
                MegaError::Process(format!("failed to start mega-get: {e}"))
            }
        })?;
        let pid = child.id().unwrap_or_default();
        info!(task_id = %task_id, pid, "mega-get started");
        let (line_tx, mut line_rx) = mpsc::channel::<(bool, String)>(128);
        let mut readers = Vec::new();
        if let Some(stdout) = child.stdout.take() {
            readers.push(spawn_line_reader(stdout, false, line_tx.clone()));
        }
        if let Some(stderr) = child.stderr.take() {
            readers.push(spawn_line_reader(stderr, true, line_tx.clone()));
        }
        drop(line_tx);

        let mut ticker = interval(Duration::from_millis(500));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let started = Instant::now();
        let mut percentage = None;
        let mut tail: Vec<String> = Vec::new();
        let mut tick_count = 0u64;
        let mut mega_progress = MegaProgress {
            process_state: "running".into(),
            temp_dir: Some(path_string(&task_dir)),
            ..Default::default()
        };
        let exit: MegaExit;
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    exit = MegaExit::Cancelled;
                    break;
                }
                Some((_is_err, line)) = line_rx.recv() => {
                    let clean = strip_ansi(&line);
                    if let Some(p) = parse_percent(&clean) {
                        percentage = Some(p);
                    }
                    if !clean.trim().is_empty() {
                        tail.push(clean);
                        if tail.len() > 24 {
                            tail.remove(0);
                        }
                    }
                }
                status = child.wait() => {
                    exit = MegaExit::Completed(status);
                    break;
                }
                _ = ticker.tick() => {
                    tick_count += 1;
                    let size = dir_size(&task_dir);
                    progress.set_temp_bytes(size);
                    progress.set_downloaded(size.max(0) as u64);
                    mega_progress.temp_bytes = size;
                    mega_progress.percentage = percentage;
                    progress.set_mega(Some(mega_progress.clone()));
                    if settings.mega_max_task_bytes > 0 && size > settings.mega_max_task_bytes {
                        exit = MegaExit::Limit(MegaError::TaskDiskLimit(settings.mega_max_task_bytes));
                        break;
                    }
                    if tick_count.is_multiple_of(8)
                        && settings.mega_temp_max_bytes > 0
                        && directory_total(Path::new(&settings.mega_temp_dir)) > settings.mega_temp_max_bytes
                    {
                        exit = MegaExit::Limit(MegaError::GlobalDiskLimit(settings.mega_temp_max_bytes));
                        break;
                    }
                    if started.elapsed() > Duration::from_secs(settings.mega_download_timeout_secs as u64) {
                        exit = MegaExit::Limit(MegaError::Timeout);
                        break;
                    }
                }
            }
        }

        if !matches!(&exit, MegaExit::Completed(_)) {
            terminate_process_group(&mut child, pid).await;
        }
        // Drain all buffered output. The readers terminate once the process
        // pipes reach EOF; their JoinHandles are kept so they can be aborted if
        // a provider leaves a pipe open unexpectedly.
        let drain_deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        loop {
            match tokio::time::timeout_at(drain_deadline, line_rx.recv()).await {
                Ok(Some((_e, line))) => {
                    tail.push(strip_ansi(&line));
                    if tail.len() > 24 {
                        tail.remove(0);
                    }
                }
                Ok(None) => break,
                Err(_) => break,
            }
        }
        for reader in readers {
            reader.abort();
        }
        let status = match exit {
            MegaExit::Cancelled => {
                if let Err(err) = self.cancel_server_transfers(settings, &task_dir).await {
                    warn!(error = %err, "failed to cancel server-side MEGA transfer");
                }
                let _ = std::fs::remove_dir_all(&task_dir);
                progress.set_mega(None);
                return Err(MegaError::Cancelled);
            }
            MegaExit::Limit(err) => {
                if let Err(cancel_err) = self.cancel_server_transfers(settings, &task_dir).await {
                    warn!(error = %cancel_err, "failed to cancel server-side MEGA transfer");
                }
                let _ = std::fs::remove_dir_all(&task_dir);
                progress.set_mega(None);
                return Err(err);
            }
            MegaExit::Completed(result) => result
                .map_err(|e| MegaError::Process(format!("failed to wait for mega-get: {e}")))?,
        };
        if !status.success() {
            if let Err(err) = self.cancel_server_transfers(settings, &task_dir).await {
                warn!(error = %err, "failed to cancel server-side MEGA transfer");
            }
            let detail = sanitize_process_output(&tail.join("\n"), url);
            let detail = if detail.trim().is_empty() {
                format!("mega-get exited with status {status}")
            } else {
                detail
            };
            let _ = std::fs::remove_dir_all(&task_dir);
            progress.set_mega(None);
            return Err(MegaError::Process(detail));
        }

        let size = dir_size(&task_dir);
        let files = collect_files(&task_dir)?;
        if files.is_empty() {
            let _ = std::fs::remove_dir_all(&task_dir);
            return Err(MegaError::NoFiles);
        }
        let total = sum_sizes(&files)?;
        if settings.mega_max_task_bytes > 0 && total > settings.mega_max_task_bytes {
            let _ = std::fs::remove_dir_all(&task_dir);
            return Err(MegaError::TaskDiskLimit(settings.mega_max_task_bytes));
        }
        if settings.max_file_bytes > 0 && total > settings.max_file_bytes {
            let _ = std::fs::remove_dir_all(&task_dir);
            return Err(MegaError::Process(format!(
                "downloaded file exceeds the configured file size limit of {} bytes",
                settings.max_file_bytes
            )));
        }
        let marker = Self::marker_path(&task_dir);
        std::fs::write(
            &marker,
            format!(
                "completed_at={}\nfiles={}\nbytes={}\n",
                Utc::now().to_rfc3339(),
                files.len(),
                total
            ),
        )
        .map_err(|e| MegaError::Io(format!("cannot write completion marker: {e}")))?;
        progress.set_temp(Some(path_string(&task_dir)), true, size);
        progress.set_downloaded(total.max(0) as u64);
        progress.set_total(Some(total));
        mega_progress.process_state = "completed".into();
        mega_progress.percentage = Some(100.0);
        mega_progress.temp_bytes = size;
        mega_progress.files_total = files.len();
        mega_progress.files_done = files.len();
        progress.set_mega(Some(mega_progress));
        info!(
            task_id = %task_id,
            files = files.len(),
            bytes = total,
            "mega-get completed"
        );
        Ok(MegaDownloadResult {
            files,
            total_bytes: total,
            temp_dir: task_dir,
        })
    }

    /// Cancel server-side transfers whose destination lies inside the app's
    /// temp root. This cleans up a crash without touching unrelated manual
    /// MEGAcmd transfers.
    pub async fn cancel_orphan_downloads(
        &self,
        settings: &AppSettings,
        root: &Path,
    ) -> Result<usize, String> {
        let transfers_bin = sibling_executable(&settings.mega_get_path, "mega-transfers");
        let output = timeout(
            Duration::from_secs(12),
            Command::new(&transfers_bin)
                .arg("--output-cols=TAG,DESTINYPATH")
                .arg("--col-separator=|")
                .arg("--path-display-size=4096")
                .arg("--only-downloads")
                .env("HOME", &settings.mega_home)
                .output(),
        )
        .await
        .map_err(|_| "mega-transfers timed out".to_string())?
        .map_err(|e| format!("cannot run mega-transfers: {e}"))?;
        let text = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let expected = root.to_string_lossy();
        let mut tags = Vec::new();
        for line in text.lines() {
            let Some((tag, destination)) = line.trim().split_once('|') else {
                continue;
            };
            if tag.eq_ignore_ascii_case("TAG") || tag.is_empty() {
                continue;
            }
            if destination.contains(expected.as_ref()) {
                tags.push(tag.trim().to_string());
            }
        }
        for tag in &tags {
            let _ = timeout(
                Duration::from_secs(12),
                Command::new(&transfers_bin)
                    .arg("-c")
                    .arg(tag)
                    .env("HOME", &settings.mega_home)
                    .output(),
            )
            .await;
        }
        Ok(tags.len())
    }

    /// mega-get hands the actual transfer to MEGAcmdServer. Killing the
    /// client process alone does not stop that server-side transfer, so a
    /// cancellation must also remove its transfer tags from the server.
    async fn cancel_server_transfers(
        &self,
        settings: &AppSettings,
        task_dir: &Path,
    ) -> Result<(), String> {
        let transfers_bin = sibling_executable(&settings.mega_get_path, "mega-transfers");
        let output = timeout(
            Duration::from_secs(12),
            Command::new(&transfers_bin)
                .arg("--output-cols=TAG,DESTINYPATH")
                .arg("--col-separator=|")
                .arg("--path-display-size=4096")
                .arg("--only-downloads")
                .env("HOME", &settings.mega_home)
                .output(),
        )
        .await
        .map_err(|_| "mega-transfers timed out".to_string())?
        .map_err(|e| format!("cannot run mega-transfers: {e}"))?;
        let text = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let expected = task_dir.to_string_lossy();
        let mut tags = Vec::new();
        for line in text.lines() {
            let Some((tag, destination)) = line.trim().split_once('|') else {
                continue;
            };
            if tag.eq_ignore_ascii_case("TAG") || tag.is_empty() {
                continue;
            }
            if destination.contains(expected.as_ref()) {
                tags.push(tag.trim().to_string());
            }
        }
        // If the listing format differs, cancel only transfers whose
        // destination is under the app temp root. This never touches manual
        // MEGAcmd transfers elsewhere.
        if tags.is_empty() {
            let _ = self
                .cancel_orphan_downloads(settings, Path::new(&settings.mega_temp_dir))
                .await?;
            return Ok(());
        }
        for tag in tags {
            let result = timeout(
                Duration::from_secs(12),
                Command::new(&transfers_bin)
                    .arg("-c")
                    .arg(&tag)
                    .env("HOME", &settings.mega_home)
                    .output(),
            )
            .await
            .map_err(|_| format!("cancel transfer {tag} timed out"))?
            .map_err(|e| format!("cannot cancel transfer {tag}: {e}"))?;
            if !result.status.success() {
                return Err(format!(
                    "mega-transfers could not cancel tag {tag}: {}",
                    String::from_utf8_lossy(&result.stderr).trim()
                ));
            }
        }
        sleep(Duration::from_millis(300)).await;
        Ok(())
    }

    pub async fn version(&self, settings: &AppSettings) -> Result<String, String> {
        // `mega-get --version` is not a valid command in current MEGAcmd.
        // The official package ships a dedicated `mega-version` executable.
        let version_bin = sibling_executable(&settings.mega_get_path, "mega-version");
        let output = timeout(
            Duration::from_secs(8),
            Command::new(&version_bin)
                .env("HOME", &settings.mega_home)
                .output(),
        )
        .await
        .map_err(|_| "mega-version timed out".to_string())?
        .map_err(|e| e.to_string())?;
        let mut text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text.is_empty() {
            text = String::from_utf8_lossy(&output.stderr).trim().to_string();
        }
        if text.is_empty() {
            Err("mega-version returned no version".into())
        } else {
            Ok(text.lines().next().unwrap_or_default().to_string())
        }
    }

    async fn whoami(&self, settings: &AppSettings) -> Result<WhoamiResult, String> {
        let output = timeout(
            Duration::from_secs(12),
            Command::new(&settings.mega_whoami_path)
                .env("HOME", &settings.mega_home)
                .output(),
        )
        .await
        .map_err(|_| "mega-whoami timed out".to_string())?
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "mega-whoami is not installed".to_string()
            } else {
                e.to_string()
            }
        })?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}\n{stderr}");
        let lower = combined.to_ascii_lowercase();
        let connection_error = [
            "failed to connect",
            "could not connect",
            "connection refused",
            "server not running",
            "no server",
            "failed to communicate",
        ]
        .iter()
        .any(|needle| lower.contains(needle));
        let not_logged = lower.contains("not logged")
            || lower.contains("not logged in")
            || lower.contains("you are not logged");
        let account = extract_account(&stdout);
        let logged_in = output.status.success() && account.is_some() && !not_logged;
        Ok(WhoamiResult {
            server_running: !connection_error,
            logged_in,
            account,
        })
    }

    fn check_executable(&self, path: &str) -> Result<(), String> {
        if path.contains('/') {
            let path = Path::new(path);
            if !path.is_file() {
                return Err(format!("{} does not exist", path.display()));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(path)
                    .map_err(|e| e.to_string())?
                    .permissions()
                    .mode();
                if mode & 0o111 == 0 {
                    return Err(format!("{} is not executable", path.display()));
                }
            }
            Ok(())
        } else {
            find_in_path(path)
                .map(|_| ())
                .ok_or_else(|| format!("{path} was not found in PATH"))
        }
    }
}

enum MegaExit {
    Completed(std::io::Result<std::process::ExitStatus>),
    Cancelled,
    Limit(MegaError),
}

fn spawn_line_reader<R>(
    mut reader: R,
    is_err: bool,
    tx: mpsc::Sender<(bool, String)>,
) -> tokio::task::JoinHandle<()>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    // MEGAcmd writes progress using carriage returns. Splitting on both CR
    // and LF keeps memory bounded and exposes percentages in real time.
    tokio::spawn(async move {
        let mut chunk = [0u8; 8192];
        let mut pending: Vec<u8> = Vec::with_capacity(1024);
        loop {
            let read = match reader.read(&mut chunk).await {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            };
            for &byte in &chunk[..read] {
                if byte == b'\n' || byte == b'\r' {
                    if !pending.is_empty() {
                        let line = String::from_utf8_lossy(&pending).trim().to_string();
                        pending.clear();
                        if !line.is_empty() && tx.send((is_err, line)).await.is_err() {
                            return;
                        }
                    }
                } else {
                    pending.push(byte);
                    if pending.len() >= 8192 {
                        let line = String::from_utf8_lossy(&pending).trim().to_string();
                        pending.clear();
                        if !line.is_empty() && tx.send((is_err, line)).await.is_err() {
                            return;
                        }
                    }
                }
            }
        }
        if !pending.is_empty() {
            let line = String::from_utf8_lossy(&pending).trim().to_string();
            if !line.is_empty() {
                let _ = tx.send((is_err, line)).await;
            }
        }
    })
}

async fn terminate_process_group(child: &mut tokio::process::Child, pid: u32) {
    #[cfg(unix)]
    {
        if pid > 0 {
            unsafe {
                // mega-get is started as its own process group. Killing the
                // group also stops any helper process it may have spawned.
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
        }
    }
    let _ = child.start_kill();
    let _ = timeout(Duration::from_secs(5), child.wait()).await;
}

fn extract_account(stdout: &str) -> Option<String> {
    let text = stdout.trim();
    if text.is_empty() {
        return None;
    }
    for token in text.split_whitespace() {
        let token = token.trim_matches(|c: char| matches!(c, '<' | '>' | ',' | '.' | ':'));
        if token.contains('@') && !token.eq_ignore_ascii_case("notlogged") {
            return Some(token.to_string());
        }
    }
    None
}

fn parse_percent(line: &str) -> Option<f64> {
    let percent_pos = line.find('%')?;
    let before = line[..percent_pos].trim_end();
    let start = before
        .rfind(|c: char| !(c.is_ascii_digit() || c == '.'))
        .map(|idx| idx + 1)
        .unwrap_or(0);
    before[start..]
        .parse::<f64>()
        .ok()
        .map(|v| v.clamp(0.0, 100.0))
}

fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for next in chars.by_ref() {
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else if c != '\r' {
            out.push(c);
        }
    }
    out
}

fn sanitize_process_output(output: &str, url: &str) -> String {
    let mut redacted = output.replace(url, "<mega-share-link>");
    if let Some(hash) = url.find('#') {
        let base = &url[..hash];
        redacted = redacted.replace(base, "<mega-share-link>");
    }
    redacted = redact_fragment_keys(&redacted);
    redacted
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(12)
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .take(2000)
        .collect()
}

fn redact_fragment_keys(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '#' {
            let mut skipped = 0usize;
            while let Some(&next) = chars.peek() {
                if next.is_whitespace() || matches!(next, '"' | '\'' | ')' | ']' | '}') {
                    break;
                }
                chars.next();
                skipped += 1;
            }
            if skipped > 3 {
                output.push_str("<redacted-key>");
            } else {
                output.push('#');
            }
        } else {
            output.push(c);
        }
    }
    output
}

fn collect_files(task_dir: &Path) -> Result<Vec<MegaFile>, MegaError> {
    let mut files = Vec::new();
    for entry in WalkDir::new(task_dir).follow_links(false) {
        let entry = entry.map_err(|e| MegaError::Io(e.to_string()))?;
        let path = entry.path();
        if path == task_dir {
            continue;
        }
        if path
            .file_name()
            .map(|name| name == COMPLETE_MARKER)
            .unwrap_or(false)
        {
            continue;
        }
        let file_type = entry.file_type();
        if file_type.is_symlink() {
            return Err(MegaError::Symlink(path.display().to_string()));
        }
        if !file_type.is_file() {
            continue;
        }
        let relative = path
            .strip_prefix(task_dir)
            .map_err(|_| MegaError::UnsafePath("file escaped task directory".into()))?
            .to_path_buf();
        let mut safe = PathBuf::new();
        for component in relative.components() {
            match component {
                std::path::Component::Normal(part) => {
                    let name = sanitize_filename(&part.to_string_lossy());
                    if name.is_empty() || name == "." || name == ".." {
                        return Err(MegaError::UnsafePath(
                            "could not sanitize downloaded file name".into(),
                        ));
                    }
                    safe.push(name);
                }
                _ => {
                    return Err(MegaError::UnsafePath(
                        "downloaded file has an unsafe relative path".into(),
                    ))
                }
            }
        }
        let metadata = entry
            .metadata()
            .map_err(|e| MegaError::Io(format!("cannot read metadata: {e}")))?;
        let size = metadata.len().min(i64::MAX as u64) as i64;
        let content_type = mime_guess::from_path(&relative)
            .first_raw()
            .map(ToOwned::to_owned);
        files.push(MegaFile {
            local_path: path.to_path_buf(),
            relative_path: relative,
            safe_relative_path: safe,
            size,
            content_type,
        });
    }
    let mut seen = HashSet::new();
    for file in &files {
        let key = file
            .safe_relative_path
            .to_string_lossy()
            .to_ascii_lowercase();
        if !seen.insert(key) {
            return Err(MegaError::UnsafePath(
                "two downloaded files map to the same S3 object name".into(),
            ));
        }
    }
    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(files)
}

fn sum_sizes(files: &[MegaFile]) -> Result<i64, MegaError> {
    files.iter().try_fold(0i64, |sum, file| {
        sum.checked_add(file.size)
            .ok_or_else(|| MegaError::Process("downloaded size overflow".into()))
    })
}

fn dir_has_files(path: &Path) -> bool {
    WalkDir::new(path)
        .min_depth(1)
        .into_iter()
        .filter_map(Result::ok)
        .any(|entry| {
            entry.file_type().is_file() && entry.file_name().to_string_lossy() != COMPLETE_MARKER
        })
}

pub fn dir_size(path: &Path) -> i64 {
    WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter_map(|entry| entry.metadata().ok())
        .map(|meta| meta.len().min(i64::MAX as u64) as i64)
        .sum()
}

pub fn temp_usage(root: &Path) -> (i64, usize) {
    if !root.exists() {
        return (0, 0);
    }
    let mut bytes = 0i64;
    let mut dirs = 0usize;
    for entry in WalkDir::new(root)
        .min_depth(1)
        .max_depth(1)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if entry.file_type().is_dir() {
            dirs += 1;
            bytes += dir_size(entry.path());
        }
    }
    (bytes, dirs)
}

fn directory_total(root: &Path) -> i64 {
    if !root.exists() {
        0
    } else {
        dir_size(root)
    }
}

fn enforce_global_limit(settings: &AppSettings) -> Result<(), MegaError> {
    if settings.mega_temp_max_bytes > 0 {
        let used = directory_total(Path::new(&settings.mega_temp_dir));
        if used > settings.mega_temp_max_bytes {
            return Err(MegaError::GlobalDiskLimit(settings.mega_temp_max_bytes));
        }
    }
    Ok(())
}

fn sibling_executable(path: &str, sibling: &str) -> PathBuf {
    let path = Path::new(path);
    if path.components().count() > 1 {
        path.parent()
            .map(|parent| parent.join(sibling))
            .unwrap_or_else(|| PathBuf::from(sibling))
    } else {
        PathBuf::from(sibling)
    }
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

pub async fn cleanup_task_dir(task_dir: &Path) -> Result<(), String> {
    if !task_dir.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(task_dir).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_progress_percentages() {
        assert_eq!(parse_percent("Downloading... 42.5%"), Some(42.5));
        assert_eq!(parse_percent("no percent"), None);
        assert_eq!(parse_percent("101%"), Some(100.0));
        assert_eq!(
            parse_percent("TRANSFERRING |x| (84/339 MB: 24.8 %)"),
            Some(24.8)
        );
    }

    #[test]
    fn extracts_accounts() {
        assert_eq!(
            extract_account("Account: user@example.com\n"),
            Some("user@example.com".into())
        );
        assert_eq!(extract_account("Not logged in"), None);
    }
}
