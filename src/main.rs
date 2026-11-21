use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result};
use axum::{
    extract::DefaultBodyLimit,
    http::{header, HeaderName, HeaderValue},
    middleware,
    routing::get,
    Router,
};
use fileuploader::{
    api,
    config::{create_private_dir, AppConfig, LogFormat},
    db::Store,
    models::{AppSettings, ServerEvent},
    services::{mega::MegaService, s3::S3Service, task_manager::TaskManager},
    state::AppState,
};
use tokio::sync::{broadcast, RwLock};
use tower_http::{
    compression::CompressionLayer,
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    let config = Arc::new(AppConfig::from_env()?);
    init_tracing(config.log_format);
    config.prepare_directories()?;

    let store = Store::connect(&config.database_url)
        .await
        .context("failed to initialize database")?;
    if config.seed_default_s3 {
        store
            .seed_s3_from_env()
            .await
            .context("failed to seed S3 configuration")?;
    }
    let defaults = AppSettings::with_env_defaults();
    let mut settings = store
        .load_settings(&defaults)
        .await
        .context("failed to load settings")?;
    settings.normalize_paths();
    settings
        .validate()
        .map_err(anyhow::Error::msg)
        .context("stored settings are invalid")?;
    create_private_dir(std::path::Path::new(&settings.mega_home))?;
    create_private_dir(std::path::Path::new(&settings.mega_temp_dir))?;

    let settings = Arc::new(RwLock::new(settings));
    let (events, _) = broadcast::channel::<ServerEvent>(2048);
    let s3 = Arc::new(S3Service::new());
    let mega = Arc::new(MegaService::new());
    let manager = TaskManager::new(
        store.clone(),
        settings.clone(),
        s3.clone(),
        mega.clone(),
        events.clone(),
    )
    .await;
    manager.startup().await.context("startup recovery failed")?;
    manager.start_background();

    let state = AppState {
        config: config.clone(),
        db: store,
        settings,
        s3,
        mega,
        manager,
        events,
        started_at: std::time::Instant::now(),
        version: env!("CARGO_PKG_VERSION"),
    };

    let index = config.static_dir.join("index.html");
    let static_service = ServeDir::new(&config.static_dir).fallback(ServeFile::new(index));
    let app = Router::new()
        .route("/healthz", get(api::system::healthz))
        .nest("/api", api::router())
        .fallback_service(static_service)
        .layer(DefaultBodyLimit::max(512 * 1024))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            HeaderName::from_static("content-security-policy"),
            HeaderValue::from_static(
                "default-src 'self'; connect-src 'self'; img-src 'self' data:;                  style-src 'self' 'unsafe-inline'; script-src 'self'; font-src 'self';                  frame-ancestors 'none'; base-uri 'self'; form-action 'self'",
            ),
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            fileuploader::auth::basic_auth,
        ))
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind(config.bind)
        .await
        .with_context(|| format!("failed to bind {}", config.bind))?;
    info!(
        address = %config.bind,
        data_dir = %config.data_dir.display(),
        static_dir = %config.static_dir.display(),
        "FileUploader is ready"
    );

    let shutdown_state = state.clone();
    tokio::select! {
        result = axum::serve(listener, app) => {
            result.context("HTTP server failed")?;
        }
        _ = wait_for_shutdown() => {
            warn!("shutdown requested; cancelling active tasks");
            shutdown_state.manager.cancel_all();
            for _ in 0..180 {
                if shutdown_state.manager.active_count() == 0 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
    info!("server stopped");
    Ok(())
}

fn init_tracing(format: LogFormat) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("info,aws_config=warn,aws_smithy_runtime=warn,hyper=warn")
    });
    let registry = tracing_subscriber::registry().with(filter);
    match format {
        LogFormat::Json => {
            registry
                .with(tracing_subscriber::fmt::layer().json())
                .init();
        }
        LogFormat::Pretty => {
            registry.with(tracing_subscriber::fmt::layer()).init();
        }
    }
}

async fn wait_for_shutdown() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut terminate = match signal(SignalKind::terminate()) {
            Ok(signal) => signal,
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
                return;
            }
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = terminate.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
