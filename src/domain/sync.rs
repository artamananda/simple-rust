//! Kontrak sinkronisasi dari sumber data luar (endpoint Apps Script lama).
//!
//! Domain hanya menyatakan *apa* yang dibutuhkan — "ada sumber yang bisa
//! menyerahkan sekumpulan komentar". Cara mengambilnya (HTTP, file, apa pun)
//! adalah urusan layer infrastructure.

use async_trait::async_trait;

use super::comment::NewComment;

/// Baris dari sumber yang tidak lolos validasi domain. Dilaporkan ke pemanggil,
/// bukan didiamkan, supaya data yang bermasalah di sheet bisa diperbaiki.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedComment {
    /// Nama apa adanya dari sumber (bisa kosong kalau memang itu masalahnya).
    pub name: String,
    pub reason: String,
}

/// Hasil pengambilan data dari sumber luar.
#[derive(Debug, Clone, Default)]
pub struct FetchedComments {
    pub comments: Vec<NewComment>,
    pub skipped: Vec<SkippedComment>,
}

/// Jumlah baris yang tersimpan pada satu kali sinkronisasi.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyncStats {
    pub created: u32,
    pub updated: u32,
}

/// Laporan lengkap satu kali sinkronisasi.
#[derive(Debug, Clone, Default)]
pub struct SyncReport {
    /// Banyaknya baris yang diterima dari sumber (termasuk yang dilewati).
    pub fetched: u32,
    pub created: u32,
    pub updated: u32,
    pub skipped: Vec<SkippedComment>,
}

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    /// Sumber tidak bisa dihubungi (jaringan mati, timeout, HTTP error).
    #[error("sumber data tidak bisa dihubungi: {0}")]
    Unreachable(#[source] anyhow::Error),

    /// Sumber menjawab, tapi bentuk datanya tidak seperti yang diharapkan.
    #[error("respons sumber data tidak sesuai: {0}")]
    InvalidResponse(#[source] anyhow::Error),
}

/// Port untuk sumber komentar eksternal.
#[async_trait]
pub trait CommentSource: Send + Sync + 'static {
    async fn fetch_all(&self) -> Result<FetchedComments, SourceError>;
}
