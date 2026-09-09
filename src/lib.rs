//! API komentar sederhana dengan clean architecture.
//!
//! Arah dependensi selalu ke dalam:
//!
//! ```text
//! presentation (HTTP/Axum) ─┐
//!                           ├─> application (use case) ─> domain (aturan bisnis)
//! infrastructure (Postgres)─┘
//! ```
//!
//! `domain` tidak tahu-menahu soal HTTP maupun SQL, sehingga aturan bisnisnya
//! bisa diuji tanpa menyalakan server ataupun database.

pub mod application;
pub mod config;
pub mod domain;
pub mod infrastructure;
pub mod presentation;

/// Metadata build yang disuntikkan `build.rs`.
pub mod build_info {
    pub const NAME: &str = env!("CARGO_PKG_NAME");
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
    pub const COMMIT: &str = env!("GIT_COMMIT");
    pub const BUILD_TIME: &str = env!("BUILD_TIME");
}
