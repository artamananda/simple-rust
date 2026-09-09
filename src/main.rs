//! Titik masuk aplikasi: baca konfigurasi, rakit dependensi, jalankan server.
//!
//! Semua penyusunan objek (wiring) terjadi di sini — layer lain hanya menerima
//! dependensinya lewat konstruktor.

use std::sync::Arc;

use anyhow::{Context, Result};
use simple_rust::application::{CommentService, SyncService};
use simple_rust::build_info;
use simple_rust::config::Config;
use simple_rust::infrastructure::db;
use simple_rust::infrastructure::http::AppsScriptCommentSource;
use simple_rust::infrastructure::postgres::PgCommentRepository;
use simple_rust::presentation::{AppState, SyncEndpoint, build_router};
use tokio::net::TcpListener;
use tokio::signal;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    init_tracing();

    let cfg = Config::from_env()?;
    tracing::info!(
        version = build_info::VERSION,
        commit = build_info::COMMIT,
        build_time = build_info::BUILD_TIME,
        "menyalakan {}",
        build_info::NAME
    );

    let pool = db::connect(&cfg.database).await?;
    tracing::info!("database terhubung");

    db::run_migrations(&pool).await?;
    tracing::info!("migrasi selesai");

    // Wiring: repository -> service -> state -> router.
    let repository = Arc::new(PgCommentRepository::new(pool.clone()));
    let service = Arc::new(CommentService::new(repository.clone()));

    let sync = match &cfg.sync {
        Some(sync_cfg) => {
            let source = Arc::new(AppsScriptCommentSource::new(
                sync_cfg.source_url.clone(),
                sync_cfg.timeout,
            )?);
            tracing::info!(url = %sync_cfg.source_url, "endpoint sync aktif");

            Some(SyncEndpoint {
                service: Arc::new(SyncService::new(source, repository)),
                secret: Arc::from(sync_cfg.secret.as_str()),
            })
        }
        None => {
            tracing::info!("endpoint sync nonaktif (SYNC_SOURCE_URL/SYNC_SECRET belum diisi)");
            None
        }
    };

    let router = build_router(AppState::new(service, sync), &cfg.http);

    let listener = TcpListener::bind(&cfg.http.addr)
        .await
        .with_context(|| format!("gagal listen di {}", cfg.http.addr))?;
    tracing::info!(addr = %cfg.http.addr, origins = ?cfg.http.allowed_origins, "api siap menerima request");

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server berhenti karena error")?;

    // Tutup pool agar koneksi dilepas rapi sebelum proses keluar.
    pool.close().await;
    tracing::info!("server berhenti");

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,tower_http=info,sqlx=warn"));

    tracing_subscriber::fmt().with_env_filter(filter).init();
}

/// Menunggu Ctrl+C atau SIGTERM (dikirim systemd saat `systemctl restart`)
/// supaya request yang sedang berjalan sempat selesai.
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("gagal memasang handler Ctrl+C");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("gagal memasang handler SIGTERM")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => tracing::info!("menerima Ctrl+C, mematikan server"),
        () = terminate => tracing::info!("menerima SIGTERM, mematikan server"),
    }
}
