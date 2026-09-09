//! Pool koneksi Postgres dan migrasi database.

use anyhow::{Context, Result};
use sqlx::postgres::{PgPool, PgPoolOptions};

use crate::config::DatabaseConfig;

/// Membuka pool dan memastikan database benar-benar bisa dihubungi sebelum
/// server mulai menerima request.
pub async fn connect(cfg: &DatabaseConfig) -> Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(cfg.max_connections)
        .min_connections(cfg.min_connections)
        .acquire_timeout(cfg.acquire_timeout)
        .connect(&cfg.url)
        .await
        .context("gagal membuka koneksi ke Postgres")?;

    sqlx::query("SELECT 1")
        .execute(&pool)
        .await
        .context("gagal ping ke Postgres")?;

    Ok(pool)
}

/// Menjalankan migrasi yang ikut ter-embed di dalam binary (folder `migrations/`),
/// jadi deploy cukup mengirim satu file biner tanpa file SQL terpisah.
pub async fn run_migrations(pool: &PgPool) -> Result<()> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .context("migrasi database gagal")?;
    Ok(())
}
