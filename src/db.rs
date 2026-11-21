use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    FromRow, Row, SqlitePool,
};
use std::{str::FromStr, time::Duration};

use crate::models::{
    AppSettings, AuthAccount, S3ConfigRecord, TaskHistoryItem, TaskRecord, TaskStatus,
};

#[derive(Clone)]
pub struct Store {
    pub pool: SqlitePool,
}

#[derive(Debug, FromRow)]
struct SettingsRow {
    value: String,
}

#[derive(Debug, FromRow)]
struct AuthAccountRow {
    username: String,
    password_hash: String,
    salt: String,
    iterations: i64,
    created_at: String,
    updated_at: String,
}

impl From<AuthAccountRow> for AuthAccount {
    fn from(row: AuthAccountRow) -> Self {
        Self {
            username: row.username,
            password_hash: row.password_hash,
            salt: row.salt,
            iterations: row.iterations,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, FromRow)]
struct S3ConfigRow {
    id: String,
    name: String,
    endpoint: String,
    region: String,
    access_key: String,
    secret_key: String,
    bucket: String,
    default_prefix: String,
    path_style: i64,
    use_tls: i64,
    metadata_json: String,
    created_at: String,
    updated_at: String,
}

impl From<S3ConfigRow> for S3ConfigRecord {
    fn from(row: S3ConfigRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            endpoint: row.endpoint,
            region: row.region,
            access_key: row.access_key,
            secret_key: row.secret_key,
            bucket: row.bucket,
            default_prefix: row.default_prefix,
            path_style: row.path_style != 0,
            use_tls: row.use_tls != 0,
            metadata_json: row.metadata_json,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, FromRow)]
struct TaskRow {
    id: String,
    url: String,
    source_type: String,
    resolved_source: Option<String>,
    s3_config_id: String,
    bucket: String,
    object_key: Option<String>,
    filename: Option<String>,
    resolved_filename: Option<String>,
    content_type: Option<String>,
    total_bytes: Option<i64>,
    downloaded_bytes: i64,
    uploaded_bytes: i64,
    status: String,
    stage: String,
    error: Option<String>,
    s3_upload_id: Option<String>,
    s3_key: Option<String>,
    temp_dir: Option<String>,
    temp_complete: i64,
    user_agent: Option<String>,
    referer: Option<String>,
    cookie: Option<String>,
    headers_json: String,
    auth_username: Option<String>,
    auth_password: Option<String>,
    mega_password: Option<String>,
    allow_overwrite: i64,
    attempt: i64,
    created_at: String,
    updated_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
    range_supported: Option<i64>,
    temp_bytes: i64,
}

impl From<TaskRow> for TaskRecord {
    fn from(row: TaskRow) -> Self {
        Self {
            id: row.id,
            url: row.url,
            source_type: row.source_type,
            resolved_source: row.resolved_source,
            s3_config_id: row.s3_config_id,
            bucket: row.bucket,
            object_key: row.object_key,
            filename: row.filename,
            resolved_filename: row.resolved_filename,
            content_type: row.content_type,
            total_bytes: row.total_bytes,
            downloaded_bytes: row.downloaded_bytes,
            uploaded_bytes: row.uploaded_bytes,
            status: row.status,
            stage: row.stage,
            error: row.error,
            s3_upload_id: row.s3_upload_id,
            s3_key: row.s3_key,
            temp_dir: row.temp_dir,
            temp_complete: row.temp_complete != 0,
            user_agent: row.user_agent,
            referer: row.referer,
            cookie: row.cookie,
            headers_json: row.headers_json,
            auth_username: row.auth_username,
            auth_password: row.auth_password,
            mega_password: row.mega_password,
            allow_overwrite: row.allow_overwrite != 0,
            attempt: row.attempt,
            created_at: row.created_at,
            updated_at: row.updated_at,
            started_at: row.started_at,
            finished_at: row.finished_at,
            range_supported: row.range_supported.map(|v| v != 0),
            temp_bytes: row.temp_bytes,
        }
    }
}

const TASK_COLUMNS: &str = r#"
    id, url, source_type, resolved_source, s3_config_id, bucket, object_key,
    filename, resolved_filename, content_type, total_bytes, downloaded_bytes,
    uploaded_bytes, status, stage, error, s3_upload_id, s3_key, temp_dir,
    temp_complete, user_agent, referer, cookie, headers_json, auth_username,
    auth_password, mega_password, allow_overwrite, attempt, created_at, updated_at,
    started_at, finished_at, range_supported, temp_bytes
"#;

impl Store {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let options = SqliteConnectOptions::from_str(database_url)
            .with_context(|| format!("invalid DATABASE_URL: {database_url}"))?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(15))
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(10)
            .min_connections(1)
            .acquire_timeout(Duration::from_secs(15))
            .connect_with(options)
            .await
            .context("failed to open SQLite database")?;
        let store = Self { pool };
        store.init_schema().await?;
        store.restrict_database_permissions();
        Ok(store)
    }

    fn restrict_database_permissions(&self) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let path = self.pool.connect_options().get_filename().to_path_buf();
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
    }

    async fn init_schema(&self) -> Result<()> {
        let statements = [
            r#"
            CREATE TABLE IF NOT EXISTS settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
            r#"
            CREATE TABLE IF NOT EXISTS auth_account (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                username TEXT NOT NULL,
                password_hash TEXT NOT NULL,
                salt TEXT NOT NULL,
                iterations INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
            r#"
            CREATE TABLE IF NOT EXISTS s3_configs (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                endpoint TEXT NOT NULL DEFAULT '',
                region TEXT NOT NULL DEFAULT 'us-east-1',
                access_key TEXT NOT NULL,
                secret_key TEXT NOT NULL,
                bucket TEXT NOT NULL DEFAULT '',
                default_prefix TEXT NOT NULL DEFAULT '',
                path_style INTEGER NOT NULL DEFAULT 0,
                use_tls INTEGER NOT NULL DEFAULT 1,
                metadata_json TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
            r#"
            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                url TEXT NOT NULL,
                source_type TEXT NOT NULL,
                resolved_source TEXT,
                s3_config_id TEXT NOT NULL,
                bucket TEXT NOT NULL,
                object_key TEXT,
                filename TEXT,
                resolved_filename TEXT,
                content_type TEXT,
                total_bytes INTEGER,
                downloaded_bytes INTEGER NOT NULL DEFAULT 0,
                uploaded_bytes INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL,
                stage TEXT NOT NULL,
                error TEXT,
                s3_upload_id TEXT,
                s3_key TEXT,
                temp_dir TEXT,
                temp_complete INTEGER NOT NULL DEFAULT 0,
                user_agent TEXT,
                referer TEXT,
                cookie TEXT,
                headers_json TEXT NOT NULL DEFAULT '{}',
                auth_username TEXT,
                auth_password TEXT,
                mega_password TEXT,
                allow_overwrite INTEGER NOT NULL DEFAULT 0,
                attempt INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                started_at TEXT,
                finished_at TEXT,
                range_supported INTEGER,
                temp_bytes INTEGER NOT NULL DEFAULT 0
            )
            "#,
            r#"
            CREATE TABLE IF NOT EXISTS task_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                attempt INTEGER NOT NULL,
                status TEXT NOT NULL,
                stage TEXT NOT NULL,
                error TEXT,
                started_at TEXT NOT NULL,
                finished_at TEXT
            )
            "#,
            "CREATE INDEX IF NOT EXISTS idx_tasks_created ON tasks(created_at DESC)",
            "CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status)",
            "CREATE INDEX IF NOT EXISTS idx_history_task ON task_history(task_id, attempt)",
        ];
        for sql in statements {
            sqlx::query(sql).execute(&self.pool).await?;
        }
        self.migrate_task_columns().await?;
        Ok(())
    }

    /// Older databases were created before MEGA share passwords were stored
    /// separately. SQLite has no "ADD COLUMN IF NOT EXISTS", so inspect the
    /// table first.
    async fn migrate_task_columns(&self) -> Result<()> {
        let columns: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_table_info('tasks')")
                .fetch_all(&self.pool)
                .await?;
        if !columns.iter().any(|name| name == "mega_password") {
            sqlx::query("ALTER TABLE tasks ADD COLUMN mega_password TEXT")
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    pub async fn load_settings(&self, defaults: &AppSettings) -> Result<AppSettings> {
        let row = sqlx::query_as::<_, SettingsRow>("SELECT value FROM settings WHERE id = 1")
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(row) => {
                let mut settings: AppSettings =
                    serde_json::from_str(&row.value).context("invalid settings JSON")?;
                // Newly introduced values get defaults when an older JSON is loaded.
                let d = defaults.clone();
                if settings.part_size_bytes <= 0 {
                    settings.part_size_bytes = d.part_size_bytes;
                }
                Ok(settings)
            }
            None => {
                self.save_settings(defaults).await?;
                Ok(defaults.clone())
            }
        }
    }

    pub async fn save_settings(&self, settings: &AppSettings) -> Result<()> {
        let value = serde_json::to_string(settings)?;
        sqlx::query(
            "INSERT INTO settings (id, value, updated_at) VALUES (1, ?, ?)
             ON CONFLICT(id) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        )
        .bind(value)
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Authentication
    // -----------------------------------------------------------------------

    pub async fn get_auth_account(&self) -> Result<Option<AuthAccount>> {
        let row = sqlx::query_as::<_, AuthAccountRow>(
            "SELECT username, password_hash, salt, iterations, created_at, updated_at
             FROM auth_account WHERE id = 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    pub async fn create_auth_account(
        &self,
        username: &str,
        password_hash: &str,
        salt: &str,
        iterations: i64,
    ) -> Result<()> {
        let timestamp = now();
        sqlx::query(
            "INSERT INTO auth_account
             (id, username, password_hash, salt, iterations, created_at, updated_at)
             VALUES (1, ?, ?, ?, ?, ?, ?)",
        )
        .bind(username)
        .bind(password_hash)
        .bind(salt)
        .bind(iterations)
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_auth_password(
        &self,
        username: &str,
        password_hash: &str,
        salt: &str,
        iterations: i64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE auth_account
             SET username = ?, password_hash = ?, salt = ?, iterations = ?, updated_at = ?
             WHERE id = 1",
        )
        .bind(username)
        .bind(password_hash)
        .bind(salt)
        .bind(iterations)
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // S3 configurations
    // -----------------------------------------------------------------------

    pub async fn list_s3_configs(&self) -> Result<Vec<S3ConfigRecord>> {
        let rows =
            sqlx::query_as::<_, S3ConfigRow>("SELECT * FROM s3_configs ORDER BY created_at ASC")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn get_s3_config(&self, id: &str) -> Result<Option<S3ConfigRecord>> {
        let row = sqlx::query_as::<_, S3ConfigRow>("SELECT * FROM s3_configs WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(Into::into))
    }

    pub async fn get_default_s3_config(&self) -> Result<Option<S3ConfigRecord>> {
        let row = sqlx::query_as::<_, S3ConfigRow>(
            "SELECT * FROM s3_configs ORDER BY created_at ASC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    pub async fn insert_s3_config(&self, cfg: &S3ConfigRecord) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO s3_configs
               (id, name, endpoint, region, access_key, secret_key, bucket, default_prefix,
                path_style, use_tls, metadata_json, created_at, updated_at)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&cfg.id)
        .bind(&cfg.name)
        .bind(&cfg.endpoint)
        .bind(&cfg.region)
        .bind(&cfg.access_key)
        .bind(&cfg.secret_key)
        .bind(&cfg.bucket)
        .bind(&cfg.default_prefix)
        .bind(cfg.path_style as i64)
        .bind(cfg.use_tls as i64)
        .bind(&cfg.metadata_json)
        .bind(&cfg.created_at)
        .bind(&cfg.updated_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_s3_config(&self, cfg: &S3ConfigRecord) -> Result<()> {
        sqlx::query(
            r#"UPDATE s3_configs SET name = ?, endpoint = ?, region = ?, access_key = ?,
               secret_key = ?, bucket = ?, default_prefix = ?, path_style = ?, use_tls = ?,
               metadata_json = ?, updated_at = ? WHERE id = ?"#,
        )
        .bind(&cfg.name)
        .bind(&cfg.endpoint)
        .bind(&cfg.region)
        .bind(&cfg.access_key)
        .bind(&cfg.secret_key)
        .bind(&cfg.bucket)
        .bind(&cfg.default_prefix)
        .bind(cfg.path_style as i64)
        .bind(cfg.use_tls as i64)
        .bind(&cfg.metadata_json)
        .bind(&cfg.updated_at)
        .bind(&cfg.id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_s3_config(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM s3_configs WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn s3_config_in_use(&self, id: &str) -> Result<bool> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE s3_config_id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(count > 0)
    }

    // -----------------------------------------------------------------------
    // Tasks
    // -----------------------------------------------------------------------

    pub async fn insert_task(&self, task: &TaskRecord) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO tasks
               (id, url, source_type, resolved_source, s3_config_id, bucket, object_key,
                filename, resolved_filename, content_type, total_bytes, downloaded_bytes,
                uploaded_bytes, status, stage, error, s3_upload_id, s3_key, temp_dir,
                temp_complete, user_agent, referer, cookie, headers_json, auth_username,
                auth_password, mega_password, allow_overwrite, attempt, created_at, updated_at,
                started_at, finished_at, range_supported, temp_bytes)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
                       ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&task.id)
        .bind(&task.url)
        .bind(&task.source_type)
        .bind(&task.resolved_source)
        .bind(&task.s3_config_id)
        .bind(&task.bucket)
        .bind(&task.object_key)
        .bind(&task.filename)
        .bind(&task.resolved_filename)
        .bind(&task.content_type)
        .bind(task.total_bytes)
        .bind(task.downloaded_bytes)
        .bind(task.uploaded_bytes)
        .bind(&task.status)
        .bind(&task.stage)
        .bind(&task.error)
        .bind(&task.s3_upload_id)
        .bind(&task.s3_key)
        .bind(&task.temp_dir)
        .bind(task.temp_complete as i64)
        .bind(&task.user_agent)
        .bind(&task.referer)
        .bind(&task.cookie)
        .bind(&task.headers_json)
        .bind(&task.auth_username)
        .bind(&task.auth_password)
        .bind(&task.mega_password)
        .bind(task.allow_overwrite as i64)
        .bind(task.attempt)
        .bind(&task.created_at)
        .bind(&task.updated_at)
        .bind(&task.started_at)
        .bind(&task.finished_at)
        .bind(task.range_supported.map(|v| v as i64))
        .bind(task.temp_bytes)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_task(&self, id: &str) -> Result<Option<TaskRecord>> {
        let sql = format!("SELECT {TASK_COLUMNS} FROM tasks WHERE id = ?");
        let row = sqlx::query_as::<_, TaskRow>(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(Into::into))
    }

    pub async fn list_tasks(&self, limit: i64) -> Result<Vec<TaskRecord>> {
        let sql = format!("SELECT {TASK_COLUMNS} FROM tasks ORDER BY created_at DESC LIMIT ?");
        let rows = sqlx::query_as::<_, TaskRow>(&sql)
            .bind(limit.clamp(1, 1000))
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn list_active_tasks(&self) -> Result<Vec<TaskRecord>> {
        let sql =
            format!("SELECT {TASK_COLUMNS} FROM tasks WHERE status IN ('pending', 'running')");
        let rows = sqlx::query_as::<_, TaskRow>(&sql)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn count_nonterminal(&self) -> Result<i64> {
        Ok(
            sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE status IN ('pending', 'running')")
                .fetch_one(&self.pool)
                .await?,
        )
    }

    pub async fn task_counts(&self) -> Result<crate::models::TaskCounts> {
        let rows = sqlx::query("SELECT status, COUNT(*) AS c FROM tasks GROUP BY status")
            .fetch_all(&self.pool)
            .await?;
        let mut counts = crate::models::TaskCounts::default();
        for row in rows {
            let status: String = row.try_get("status")?;
            let c: i64 = row.try_get("c")?;
            counts.total += c;
            match TaskStatus::parse(&status) {
                TaskStatus::Pending => counts.pending += c,
                TaskStatus::Running => counts.running += c,
                TaskStatus::Completed => counts.completed += c,
                TaskStatus::Failed => counts.failed += c,
                TaskStatus::Cancelled => counts.cancelled += c,
            }
        }
        Ok(counts)
    }

    pub async fn update_task_status(
        &self,
        id: &str,
        status: TaskStatus,
        stage: crate::models::TaskStage,
        error: Option<&str>,
        finished: bool,
    ) -> Result<()> {
        let now = now();
        sqlx::query(
            "UPDATE tasks SET status = ?, stage = ?, error = ?, updated_at = ?,
             finished_at = CASE WHEN ? THEN ? ELSE finished_at END WHERE id = ?",
        )
        .bind(status.as_str())
        .bind(stage.as_str())
        .bind(error)
        .bind(&now)
        .bind(finished as i64)
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_started(&self, id: &str) -> Result<()> {
        let now = now();
        sqlx::query(
            "UPDATE tasks SET status = 'running', started_at = COALESCE(started_at, ?),
             updated_at = ? WHERE id = ?",
        )
        .bind(&now)
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_task_source(
        &self,
        id: &str,
        resolved_source: &str,
        filename: Option<&str>,
        content_type: Option<&str>,
        total_bytes: Option<i64>,
        range_supported: Option<bool>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE tasks SET resolved_source = ?, resolved_filename = COALESCE(?, resolved_filename),
             content_type = COALESCE(?, content_type), total_bytes = COALESCE(?, total_bytes),
             range_supported = ?, updated_at = ? WHERE id = ?",
        )
        .bind(resolved_source)
        .bind(filename)
        .bind(content_type)
        .bind(total_bytes)
        .bind(range_supported.map(|v| v as i64))
        .bind(now())
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_task_upload(
        &self,
        id: &str,
        upload_id: Option<&str>,
        s3_key: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE tasks SET s3_upload_id = ?, s3_key = COALESCE(?, s3_key), updated_at = ? WHERE id = ?",
        )
        .bind(upload_id)
        .bind(s3_key)
        .bind(now())
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_task_temp(
        &self,
        id: &str,
        temp_dir: Option<&str>,
        temp_complete: bool,
        temp_bytes: i64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE tasks SET temp_dir = COALESCE(?, temp_dir), temp_complete = ?, temp_bytes = ?,
             updated_at = ? WHERE id = ?",
        )
        .bind(temp_dir)
        .bind(temp_complete as i64)
        .bind(temp_bytes)
        .bind(now())
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn clear_task_temp(&self, id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE tasks SET temp_dir = NULL, temp_complete = 0, temp_bytes = 0, updated_at = ?
             WHERE id = ?",
        )
        .bind(now())
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_task_counters(
        &self,
        id: &str,
        downloaded: i64,
        uploaded: i64,
        total: Option<i64>,
        temp_bytes: i64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE tasks SET downloaded_bytes = ?, uploaded_bytes = ?, total_bytes = COALESCE(?, total_bytes),
             temp_bytes = ?, updated_at = ? WHERE id = ?",
        )
        .bind(downloaded)
        .bind(uploaded)
        .bind(total)
        .bind(temp_bytes)
        .bind(now())
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn reset_task_for_retry(
        &self,
        id: &str,
        resolved_source: Option<&str>,
        s3_config_id: &str,
        bucket: &str,
        object_key: Option<&str>,
    ) -> Result<()> {
        let now = now();
        sqlx::query(
            r#"UPDATE tasks SET
                status = 'pending', stage = 'pending', error = NULL, downloaded_bytes = 0,
                uploaded_bytes = 0, total_bytes = NULL, s3_upload_id = NULL, s3_key = NULL,
                resolved_source = COALESCE(?, resolved_source), s3_config_id = ?, bucket = ?,
                object_key = ?, range_supported = NULL, attempt = attempt + 1,
                updated_at = ?, started_at = NULL, finished_at = NULL
               WHERE id = ?"#,
        )
        .bind(resolved_source)
        .bind(s3_config_id)
        .bind(bucket)
        .bind(object_key)
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_task(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM tasks WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn insert_history_start(
        &self,
        task_id: &str,
        attempt: i64,
        status: &str,
        stage: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO task_history (task_id, attempt, status, stage, started_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(task_id)
        .bind(attempt)
        .bind(status)
        .bind(stage)
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn finish_history(
        &self,
        task_id: &str,
        attempt: i64,
        status: &str,
        stage: &str,
        error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE task_history SET status = ?, stage = ?, error = ?, finished_at = ?
             WHERE task_id = ? AND attempt = ? AND finished_at IS NULL",
        )
        .bind(status)
        .bind(stage)
        .bind(error)
        .bind(now())
        .bind(task_id)
        .bind(attempt)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_history(&self, task_id: &str) -> Result<Vec<TaskHistoryItem>> {
        let rows = sqlx::query(
            "SELECT id, attempt, status, stage, error, started_at, finished_at
             FROM task_history WHERE task_id = ? ORDER BY attempt DESC, id DESC",
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| TaskHistoryItem {
                id: row.get("id"),
                attempt: row.get("attempt"),
                status: row.get("status"),
                stage: row.get("stage"),
                error: row.get("error"),
                started_at: row.get("started_at"),
                finished_at: row.get("finished_at"),
            })
            .collect())
    }

    pub async fn seed_s3_from_env(&self) -> Result<()> {
        if !self.list_s3_configs().await?.is_empty() {
            return Ok(());
        }
        let access = std::env::var("S3_ACCESS_KEY").unwrap_or_default();
        let secret = std::env::var("S3_SECRET_KEY").unwrap_or_default();
        if access.trim().is_empty() || secret.trim().is_empty() {
            return Ok(());
        }
        let id = std::env::var("S3_CONFIG_ID").unwrap_or_else(|_| "default".into());
        let now = now();
        let cfg = S3ConfigRecord {
            id,
            name: std::env::var("S3_CONFIG_NAME").unwrap_or_else(|_| "Default S3".into()),
            endpoint: std::env::var("S3_ENDPOINT").unwrap_or_default(),
            region: std::env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".into()),
            access_key: access,
            secret_key: secret,
            bucket: std::env::var("S3_BUCKET").unwrap_or_default(),
            default_prefix: std::env::var("S3_DEFAULT_PREFIX").unwrap_or_default(),
            path_style: env_bool("S3_PATH_STYLE", false),
            use_tls: env_bool("S3_USE_TLS", true),
            metadata_json: "{}".into(),
            created_at: now.clone(),
            updated_at: now,
        };
        self.insert_s3_config(&cfg).await?;
        tracing::info!(config_id = %cfg.id, "seeded default S3 configuration from environment");
        Ok(())
    }

    pub async fn mark_interrupted_tasks(&self) -> Result<u64> {
        let active = self.list_active_tasks().await?;
        for task in &active {
            self.update_task_status(
                &task.id,
                TaskStatus::Failed,
                crate::models::TaskStage::Failed,
                Some("Task was interrupted by a server restart. Use Retry to resume."),
                true,
            )
            .await?;
            let _ = self
                .finish_history(
                    &task.id,
                    task.attempt,
                    "failed",
                    "failed",
                    Some("Interrupted by server restart"),
                )
                .await;
        }
        Ok(active.len() as u64)
    }
}

pub fn now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .map(|v| !matches!(v.as_str(), "0" | "false" | "no"))
        .unwrap_or(default)
}
