//! State yang dibagikan ke seluruh handler.

use std::sync::Arc;

use chrono::FixedOffset;

use crate::application::{CommentService, SyncService};

#[derive(Clone)]
pub struct AppState {
    pub comments: Arc<CommentService>,
    /// `None` kalau SYNC_SOURCE_URL/SYNC_SECRET belum diisi — endpoint sync
    /// menjawab 503, bukan berjalan dengan kunci default.
    pub sync: Option<SyncEndpoint>,
    /// Zona waktu untuk menafsirkan tanggal tanpa zona di endpoint `/exec`.
    pub legacy_offset: FixedOffset,
}

/// Use case sync beserta kata kunci yang menjaganya.
#[derive(Clone)]
pub struct SyncEndpoint {
    pub service: Arc<SyncService>,
    pub secret: Arc<str>,
}

impl AppState {
    pub fn new(
        comments: Arc<CommentService>,
        sync: Option<SyncEndpoint>,
        legacy_offset: FixedOffset,
    ) -> Self {
        Self {
            comments,
            sync,
            legacy_offset,
        }
    }
}
