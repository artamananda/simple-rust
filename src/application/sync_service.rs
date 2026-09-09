//! Use case sinkronisasi: tarik komentar dari sumber luar, simpan dengan nama
//! sebagai kunci.
//!
//! Service ini hanya mengenal dua trait (`CommentSource` dan
//! `CommentRepository`), jadi bisa diuji tanpa jaringan maupun database.

use std::sync::Arc;

use super::error::ServiceResult;
use crate::domain::{CommentRepository, CommentSource, SyncReport};

pub struct SyncService {
    source: Arc<dyn CommentSource>,
    repository: Arc<dyn CommentRepository>,
}

impl SyncService {
    pub fn new(source: Arc<dyn CommentSource>, repository: Arc<dyn CommentRepository>) -> Self {
        Self { source, repository }
    }

    pub async fn run(&self) -> ServiceResult<SyncReport> {
        let fetched = self.source.fetch_all().await?;

        let total = fetched.comments.len() + fetched.skipped.len();
        let stats = self
            .repository
            .upsert_many_by_name(&fetched.comments)
            .await?;

        Ok(SyncReport {
            fetched: total as u32,
            created: stats.created,
            updated: stats.updated,
            skipped: fetched.skipped,
        })
    }
}
