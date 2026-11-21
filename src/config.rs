use std::{
    env,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub bind: SocketAddr,
    pub data_dir: PathBuf,
    pub database_url: String,
    pub static_dir: PathBuf,
    pub log_format: LogFormat,
    pub seed_default_s3: bool,
    pub basic_auth: Option<BasicAuthConfig>,
}

#[derive(Clone)]
pub struct BasicAuthConfig {
    pub username: String,
    pub password: String,
}

impl std::fmt::Debug for BasicAuthConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BasicAuthConfig")
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

impl BasicAuthConfig {
    pub fn from_env() -> Result<Option<Self>> {
        let username = env::var("APP_BASIC_AUTH_USERNAME")
            .ok()
            .filter(|value| !value.is_empty());
        let password = env::var("APP_BASIC_AUTH_PASSWORD")
            .ok()
            .filter(|value| !value.is_empty());
        match (username, password) {
            (None, None) => Ok(None),
            (Some(_), None) | (None, Some(_)) => Err(anyhow::anyhow!(
                "APP_BASIC_AUTH_USERNAME and APP_BASIC_AUTH_PASSWORD must be set together"
            )),
            (Some(username), Some(password)) => {
                if username.contains(':') {
                    return Err(anyhow::anyhow!(
                        "APP_BASIC_AUTH_USERNAME cannot contain ':'"
                    ));
                }
                Ok(Some(Self { username, password }))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Pretty,
    Json,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let host = env::var("APP_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port: u16 = env::var("APP_PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse()
            .context("APP_PORT must be a valid u16")?;
        let ip: IpAddr = host
            .parse()
            .with_context(|| format!("APP_HOST must be an IP address, got {host}"))?;

        let data_dir = PathBuf::from(env::var("DATA_DIR").unwrap_or_else(|_| "./data".into()));
        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
            format!(
                "sqlite://{}",
                data_dir.join("fileuploader.db").to_string_lossy()
            )
        });
        let static_dir =
            PathBuf::from(env::var("STATIC_DIR").unwrap_or_else(|_| "./frontend/dist".into()));
        let log_format = match env::var("LOG_FORMAT").as_deref() {
            Ok("json") => LogFormat::Json,
            _ => LogFormat::Pretty,
        };

        Ok(Self {
            bind: SocketAddr::new(ip, port),
            data_dir,
            database_url,
            static_dir,
            log_format,
            seed_default_s3: env::var("SEED_DEFAULT_S3")
                .map(|v| !matches!(v.as_str(), "0" | "false" | "no"))
                .unwrap_or(true),
            basic_auth: BasicAuthConfig::from_env()?,
        })
    }

    pub fn prepare_directories(&self) -> Result<()> {
        create_private_dir(&self.data_dir)?;
        if let Some(parent) = sqlite_file_path(&self.database_url) {
            if let Some(parent) = parent.parent() {
                create_private_dir(parent)?;
            }
        }
        Ok(())
    }
}

fn sqlite_file_path(database_url: &str) -> Option<PathBuf> {
    let raw = database_url
        .strip_prefix("sqlite://")
        .or_else(|| database_url.strip_prefix("sqlite:"))?;
    if raw == ":memory:" || raw.is_empty() {
        return None;
    }
    Some(PathBuf::from(raw.split('?').next().unwrap_or(raw)))
}

/// Create a directory and restrict it to the current user on Unix.
pub fn create_private_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)
        .with_context(|| format!("failed to create directory {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .with_context(|| format!("failed to set permissions on {}", path.display()))?;
    }
    Ok(())
}
