//! Bentuk JSON yang dilihat klien, terpisah dari entity domain.
//!
//! Pemisahan ini yang membuat kolom database (`commented_at`) dan field API
//! (`date`) bisa berbeda tanpa saling mengikat.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{
    Comment, DomainError, ListCommentsQuery, NewComment, Pagination, SortOrder, UpdateComment,
};

/// Body untuk `POST /api/comments`.
///
/// Semua field sengaja `Option` agar pesan error datang dari validasi domain
/// ("name: wajib diisi"), bukan dari serde. Field asing (mis. `id` bawaan versi
/// Apps Script) diabaikan, jadi klien lama tidak langsung patah.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCommentRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub date: Option<DateTime<Utc>>,
}

impl CreateCommentRequest {
    pub fn into_domain(self) -> Result<NewComment, DomainError> {
        NewComment::new(
            self.name.as_deref().unwrap_or_default(),
            self.status.as_deref().unwrap_or_default(),
            self.message.as_deref().unwrap_or_default(),
            self.color.as_deref(),
            self.date,
        )
    }
}

/// Body untuk `PUT /api/comments/{id}` — hanya field yang dikirim yang berubah.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCommentRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub date: Option<DateTime<Utc>>,
}

impl UpdateCommentRequest {
    pub fn into_domain(self) -> Result<UpdateComment, DomainError> {
        UpdateComment::new(
            self.name.as_deref(),
            self.status.as_deref(),
            self.message.as_deref(),
            self.color.as_deref(),
            self.date,
        )
    }
}

/// Query string untuk `GET /api/comments`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListCommentsParams {
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default, alias = "per_page", alias = "limit")]
    pub per_page: Option<u32>,
    #[serde(default)]
    pub status: Option<String>,
    /// Kata kunci pencarian pada nama atau pesan.
    #[serde(default, alias = "search")]
    pub q: Option<String>,
    /// `newest` (default) atau `oldest`.
    #[serde(default)]
    pub sort: Option<String>,
}

impl ListCommentsParams {
    pub fn into_domain(self) -> Result<ListCommentsQuery, DomainError> {
        let pagination = Pagination::new(
            self.page.unwrap_or(1),
            self.per_page.unwrap_or(Pagination::DEFAULT_PER_PAGE),
        )?;
        let sort = self
            .sort
            .as_deref()
            .map(SortOrder::parse)
            .transpose()?
            .unwrap_or_default();

        Ok(ListCommentsQuery {
            pagination,
            status: non_empty(self.status),
            search: non_empty(self.q),
            sort,
        })
    }
}

/// Bentuk komentar yang dikirim ke klien.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentResponse {
    pub id: Uuid,
    pub name: String,
    pub status: String,
    pub message: String,
    pub color: String,
    pub date: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<&Comment> for CommentResponse {
    fn from(comment: &Comment) -> Self {
        Self {
            id: comment.id,
            name: comment.name.as_str().to_owned(),
            status: comment.status.as_str().to_owned(),
            message: comment.message.as_str().to_owned(),
            color: comment.color.as_str().to_owned(),
            date: comment.date,
            created_at: comment.created_at,
            updated_at: comment.updated_at,
        }
    }
}

impl From<Comment> for CommentResponse {
    fn from(comment: Comment) -> Self {
        Self::from(&comment)
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_kosong_memakai_default() {
        let query = ListCommentsParams::default()
            .into_domain()
            .expect("default valid");

        assert_eq!(query.pagination.page(), 1);
        assert_eq!(query.pagination.per_page(), Pagination::DEFAULT_PER_PAGE);
        assert_eq!(query.sort, SortOrder::Newest);
        assert!(query.status.is_none());
    }

    #[test]
    fn filter_berisi_spasi_dianggap_kosong() {
        let params = ListCommentsParams {
            status: Some("   ".to_owned()),
            ..Default::default()
        };

        assert!(params.into_domain().expect("valid").status.is_none());
    }

    #[test]
    fn sort_tidak_dikenal_ditolak() {
        let params = ListCommentsParams {
            sort: Some("acak".to_owned()),
            ..Default::default()
        };

        assert!(params.into_domain().is_err());
    }
}
