use std::{sync::Arc, time::Instant};

use tokio::sync::{broadcast, RwLock};

use crate::{
    config::AppConfig,
    db::Store,
    models::{AppSettings, ServerEvent},
    services::{mega::MegaService, s3::S3Service, task_manager::TaskManager},
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub db: Store,
    pub settings: Arc<RwLock<AppSettings>>,
    pub s3: Arc<S3Service>,
    pub mega: Arc<MegaService>,
    pub manager: TaskManager,
    pub events: broadcast::Sender<ServerEvent>,
    pub started_at: Instant,
    pub version: &'static str,
}
