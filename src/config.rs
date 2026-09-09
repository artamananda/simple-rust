//! Konfigurasi aplikasi — satu-satunya tempat yang membaca environment variable.

use std::time::Duration;

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone)]
pub struct Config {
    pub http: HttpConfig,
    pub database: DatabaseConfig,
    /// `None` kalau endpoint sync belum dikonfigurasi — fiturnya mati, bukan
    /// jalan dengan nilai default yang bisa ditebak orang.
    pub sync: Option<SyncConfig>,
}

#[derive(Debug, Clone)]
pub struct HttpConfig {
    pub addr: String,
    pub allowed_origins: Vec<String>,
    pub request_timeout: Duration,
}

/// Konfigurasi endpoint sinkronisasi dari sumber luar.
#[derive(Clone)]
pub struct SyncConfig {
    pub source_url: String,
    /// Kata kunci yang harus dikirim di body agar sync boleh berjalan.
    pub secret: String,
    /// Apps Script bisa menjawab sangat lambat, jadi timeout-nya longgar.
    pub timeout: Duration,
}

// Ditulis manual agar secret tidak ikut tercetak di log saat Config di-debug.
impl std::fmt::Debug for SyncConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncConfig")
            .field("source_url", &self.source_url)
            .field("secret", &"<disembunyikan>")
            .field("timeout", &self.timeout)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_timeout: Duration,
}

impl Config {
    /// Membaca konfigurasi dari environment. Gagal lebih awal (sebelum server
    /// menyala) kalau ada nilai wajib yang belum diisi.
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            http: HttpConfig {
                addr: normalize_addr(&optional("API_ADDR", "0.0.0.0:8080")),
                allowed_origins: parse_origins(&optional("WEB_ORIGIN", "*")),
                request_timeout: Duration::from_secs(number("REQUEST_TIMEOUT_SECONDS", 30)?),
            },
            database: DatabaseConfig {
                url: required("DATABASE_URL")?,
                max_connections: number("DB_MAX_CONNECTIONS", 10)? as u32,
                min_connections: number("DB_MIN_CONNECTIONS", 1)? as u32,
                acquire_timeout: Duration::from_secs(number("DB_ACQUIRE_TIMEOUT_SECONDS", 10)?),
            },
            sync: sync_from_env()?,
        })
    }
}

/// Sync hanya aktif kalau URL sumber **dan** kata kuncinya sama-sama diisi.
fn sync_from_env() -> Result<Option<SyncConfig>> {
    let source_url = std::env::var("SYNC_SOURCE_URL").unwrap_or_default();
    let secret = std::env::var("SYNC_SECRET").unwrap_or_default();

    if source_url.trim().is_empty() || secret.trim().is_empty() {
        return Ok(None);
    }

    Ok(Some(SyncConfig {
        source_url: source_url.trim().to_owned(),
        secret: secret.trim().to_owned(),
        timeout: Duration::from_secs(number("SYNC_TIMEOUT_SECONDS", 120)?),
    }))
}

fn required(key: &'static str) -> Result<String> {
    match std::env::var(key) {
        Ok(value) if !value.trim().is_empty() => Ok(value.trim().to_owned()),
        _ => bail!("{key} wajib diisi — salin .env.example menjadi .env lalu isi nilainya"),
    }
}

fn optional(key: &str, fallback: &str) -> String {
    match std::env::var(key) {
        Ok(value) if !value.trim().is_empty() => value.trim().to_owned(),
        _ => fallback.to_owned(),
    }
}

fn number(key: &'static str, fallback: u64) -> Result<u64> {
    match std::env::var(key) {
        Ok(value) if !value.trim().is_empty() => value
            .trim()
            .parse()
            .with_context(|| format!("{key} harus berupa angka")),
        _ => Ok(fallback),
    }
}

/// Menerima gaya penulisan `:8080` (seperti service Go di novelle) dan
/// menerjemahkannya ke `0.0.0.0:8080` yang dibutuhkan `TcpListener`.
fn normalize_addr(raw: &str) -> String {
    match raw.strip_prefix(':') {
        Some(port) => format!("0.0.0.0:{port}"),
        None => raw.to_owned(),
    }
}

fn parse_origins(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addr_gaya_go_diterjemahkan() {
        assert_eq!(normalize_addr(":8080"), "0.0.0.0:8080");
        assert_eq!(normalize_addr("127.0.0.1:9000"), "127.0.0.1:9000");
    }

    #[test]
    fn origin_dipisah_koma() {
        assert_eq!(
            parse_origins("http://localhost:5173, https://artamananda.my.id ,"),
            vec![
                "http://localhost:5173".to_owned(),
                "https://artamananda.my.id".to_owned()
            ]
        );
    }
}
