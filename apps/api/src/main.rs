mod adapter;
mod db;
mod error;
mod indicators;
mod ingest;
mod live;
mod market;
mod models;
mod repository;
mod routes;
mod security;
mod seed;
mod sources;
mod sync;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use chrono::{Datelike, Local, Timelike};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("alphak_api=info".parse().unwrap()),
        )
        .init();

    let bind = std::env::var("ALPHAK_BIND").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let addr: SocketAddr = bind.parse().expect("ALPHAK_BIND must be host:port");
    if !addr.ip().is_loopback() {
        eprintln!("AlphaK MVP only supports loopback binding; got {addr}. Refusing to start.");
        std::process::exit(2);
    }

    let db_path = std::env::var("ALPHAK_DB").unwrap_or_else(|_| "alphak.db".into());
    if std::env::var_os("ALPHAK_BACKUP_BEFORE_START").is_some()
        && std::path::Path::new(&db_path).is_file()
    {
        let backup = format!(
            "backups/pre-migrate-{}.db",
            chrono::Utc::now().format("%Y%m%d-%H%M%S-%f")
        );
        let status = std::process::Command::new("bash")
            .arg("scripts/backup.sh")
            .arg(backup)
            .env("ALPHAK_DB", &db_path)
            .status()
            .expect("failed to run pre-migration backup");
        assert!(
            status.success(),
            "pre-migration backup failed; refusing to start"
        );
    }
    let conn = db::init_db(&db_path).expect("failed to initialise sqlite database");
    let state = db::AppState {
        db: Arc::new(Mutex::new(conn)),
        sources: Arc::new(sources::SourceHub::new()),
    };
    let mut app = routes::build_app(state.clone());
    if let Ok(directory) = std::env::var("ALPHAK_WEB_DIR") {
        let directory = std::path::PathBuf::from(directory);
        assert!(
            directory.join("index.html").is_file(),
            "ALPHAK_WEB_DIR must contain index.html"
        );
        app = app
            .route("/api", axum::routing::any(routes::fallback))
            .route("/api/{*path}", axum::routing::any(routes::fallback))
            .fallback_service(tower_http::services::ServeDir::new(&directory).fallback(
                tower_http::services::ServeFile::new(directory.join("index.html")),
            ));
    }
    spawn_scheduler(state.clone());
    routes::spawn_market_updates(state.clone());
    live::spawn_updates(state.clone());
    sources::retry::spawn(state);

    let listener = TcpListener::bind(addr).await.expect("failed to bind");
    tracing::info!("AlphaK API listening on http://{addr} · public-source market data");
    axum::serve(listener, app).await.expect("server error");
}

fn spawn_scheduler(state: db::AppState) {
    tokio::spawn(async move {
        loop {
            let now = Local::now();
            if sync::is_schedule_window(now.weekday(), now.hour(), now.minute()) {
                if let Ok(conn) = state.db.lock() {
                    let window = now.format("%Y-%m-%d-%H:%M").to_string();
                    for dataset in sync::scheduled_datasets(now.hour()) {
                        if let Err(err) = sync::run_scheduled(&conn, dataset, &window) {
                            tracing::warn!(dataset, error = %err.message, "scheduled sync failed");
                        }
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    });
}
