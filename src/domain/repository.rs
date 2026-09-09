//! Kontrak akses data. Domain hanya mendefinisikan *apa* yang dibutuhkan;
//! implementasinya (Postgres, in-memory untuk test) hidup di layer luar.

use async_trait::async_trait;
use uuid::Uuid;

use super::comment::{Comment, NewComment, UpdateComment};
use super::error::DomainError;
use super::pagination::{Page, Pagination};
use super::sync::SyncStats;

pub type RepositoryResult<T> = Result<T, RepositoryError>;

/// Kegagalan di sisi penyimpanan. Sengaja tidak menyebut `sqlx` supaya domain
/// tetap bebas dari detail driver database.
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    /// Baris tersimpan melanggar aturan domain (mis. hasil migrasi data lama).
    #[error("data tersimpan tidak valid: {0}")]
    Corrupt(#[from] DomainError),

    /// Kegagalan teknis: koneksi putus, query error, dan sejenisnya.
    #[error(transparent)]
    Backend(#[from] anyhow::Error),
}

/// Urutan hasil pada endpoint list.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SortOrder {
    /// Komentar terbaru lebih dulu (default).
    #[default]
    Newest,
    Oldest,
}

impl SortOrder {
    pub fn parse(raw: &str) -> Result<Self, DomainError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "newest" | "desc" => Ok(Self::Newest),
            "oldest" | "asc" => Ok(Self::Oldest),
            other => Err(DomainError::validation(
                "sort",
                format!("nilai '{other}' tidak dikenal (pakai newest atau oldest)"),
            )),
        }
    }
}

/// Kriteria pencarian daftar komentar.
#[derive(Debug, Clone, Default)]
pub struct ListCommentsQuery {
    pub pagination: Pagination,
    /// Filter persis pada kolom status.
    pub status: Option<String>,
    /// Pencarian bebas pada nama atau isi pesan.
    pub search: Option<String>,
    pub sort: SortOrder,
}

#[async_trait]
pub trait CommentRepository: Send + Sync + 'static {
    async fn list(&self, query: &ListCommentsQuery) -> RepositoryResult<Page<Comment>>;

    async fn find_by_id(&self, id: Uuid) -> RepositoryResult<Option<Comment>>;

    async fn create(&self, input: &NewComment) -> RepositoryResult<Comment>;

    /// `None` kalau id tidak ada.
    async fn update(&self, id: Uuid, input: &UpdateComment) -> RepositoryResult<Option<Comment>>;

    /// `false` kalau tidak ada baris yang terhapus.
    async fn delete(&self, id: Uuid) -> RepositoryResult<bool>;

    /// Menyimpan sekumpulan komentar dengan **nama sebagai kunci**: nama yang
    /// sudah ada diperbarui, yang belum ada ditambahkan. Dipakai endpoint sync
    /// agar bisa dijalankan berulang kali tanpa menggandakan data.
    async fn upsert_many_by_name(&self, inputs: &[NewComment]) -> RepositoryResult<SyncStats>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_order_dari_string() {
        assert_eq!(
            SortOrder::parse("newest").expect("valid"),
            SortOrder::Newest
        );
        assert_eq!(SortOrder::parse(" ASC ").expect("valid"), SortOrder::Oldest);
        assert!(SortOrder::parse("random").is_err());
    }
}
