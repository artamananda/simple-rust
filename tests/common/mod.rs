//! Perkakas bersama untuk test: repository in-memory dan sumber palsu.
//!
//! Keduanya mengimplementasikan trait domain, jadi seluruh use case bisa diuji
//! tanpa Postgres dan tanpa jaringan.
#![allow(dead_code)]

use std::sync::Mutex;

use async_trait::async_trait;
use chrono::Utc;
use simple_rust::domain::{
    Comment, CommentRepository, CommentSource, FetchedComments, ListCommentsQuery, NewComment,
    Page, RepositoryResult, SkippedComment, SortOrder, SourceError, SyncStats, UpdateComment,
};
use uuid::Uuid;

#[derive(Default)]
pub struct InMemoryCommentRepository {
    items: Mutex<Vec<Comment>>,
}

impl InMemoryCommentRepository {
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Comment>> {
        self.items.lock().expect("mutex tidak diracuni")
    }
}

#[async_trait]
impl CommentRepository for InMemoryCommentRepository {
    async fn list(&self, query: &ListCommentsQuery) -> RepositoryResult<Page<Comment>> {
        let mut items: Vec<Comment> = self
            .lock()
            .iter()
            .filter(|comment| match &query.status {
                Some(status) => comment.status.as_str() == status,
                None => true,
            })
            .cloned()
            .collect();

        match query.sort {
            SortOrder::Newest => items.sort_by_key(|item| std::cmp::Reverse(item.date)),
            SortOrder::Oldest => items.sort_by_key(|item| item.date),
        }

        let total = items.len() as i64;
        let page: Vec<Comment> = items
            .into_iter()
            .skip(query.pagination.offset() as usize)
            .take(query.pagination.limit() as usize)
            .collect();

        Ok(Page::new(page, total, query.pagination))
    }

    async fn list_all(&self) -> RepositoryResult<Vec<Comment>> {
        let mut items = self.lock().clone();
        items.sort_by_key(|item| item.date);

        Ok(items)
    }

    async fn find_by_id(&self, id: Uuid) -> RepositoryResult<Option<Comment>> {
        Ok(self.lock().iter().find(|comment| comment.id == id).cloned())
    }

    async fn create(&self, input: &NewComment) -> RepositoryResult<Comment> {
        let now = Utc::now();
        let comment = Comment {
            id: Uuid::new_v4(),
            name: input.name.clone(),
            status: input.status.clone(),
            message: input.message.clone(),
            color: input.color.clone(),
            date: input.date,
            created_at: now,
            updated_at: now,
        };

        self.lock().push(comment.clone());
        Ok(comment)
    }

    async fn update(&self, id: Uuid, input: &UpdateComment) -> RepositoryResult<Option<Comment>> {
        let mut items = self.lock();
        let Some(comment) = items.iter_mut().find(|comment| comment.id == id) else {
            return Ok(None);
        };

        if let Some(name) = input.name.clone() {
            comment.name = name;
        }
        if let Some(status) = input.status.clone() {
            comment.status = status;
        }
        if let Some(message) = input.message.clone() {
            comment.message = message;
        }
        if let Some(color) = input.color.clone() {
            comment.color = color;
        }
        if let Some(date) = input.date {
            comment.date = date;
        }
        comment.updated_at = Utc::now();

        Ok(Some(comment.clone()))
    }

    async fn delete(&self, id: Uuid) -> RepositoryResult<bool> {
        let mut items = self.lock();
        let before = items.len();
        items.retain(|comment| comment.id != id);

        Ok(items.len() != before)
    }

    async fn upsert_many_by_name(&self, inputs: &[NewComment]) -> RepositoryResult<SyncStats> {
        let mut stats = SyncStats::default();

        for input in inputs {
            let key = input.name.as_str().trim().to_lowercase();
            let mut items = self.lock();

            match items
                .iter_mut()
                .find(|comment| comment.name.as_str().trim().to_lowercase() == key)
            {
                Some(existing) => {
                    existing.name = input.name.clone();
                    existing.status = input.status.clone();
                    existing.message = input.message.clone();
                    existing.color = input.color.clone();
                    existing.date = input.date;
                    existing.updated_at = Utc::now();
                    stats.updated += 1;
                }
                None => {
                    let now = Utc::now();
                    items.push(Comment {
                        id: Uuid::new_v4(),
                        name: input.name.clone(),
                        status: input.status.clone(),
                        message: input.message.clone(),
                        color: input.color.clone(),
                        date: input.date,
                        created_at: now,
                        updated_at: now,
                    });
                    stats.created += 1;
                }
            }
        }

        Ok(stats)
    }
}

/// Sumber palsu: mengembalikan apa yang disiapkan test, tanpa menyentuh jaringan.
pub struct FakeSource {
    result: Mutex<Option<Result<FetchedComments, SourceError>>>,
}

impl FakeSource {
    pub fn returning(comments: Vec<NewComment>, skipped: Vec<SkippedComment>) -> Self {
        Self {
            result: Mutex::new(Some(Ok(FetchedComments { comments, skipped }))),
        }
    }

    pub fn failing() -> Self {
        Self {
            result: Mutex::new(Some(Err(SourceError::Unreachable(anyhow::anyhow!(
                "koneksi timeout"
            ))))),
        }
    }
}

#[async_trait]
impl CommentSource for FakeSource {
    async fn fetch_all(&self) -> Result<FetchedComments, SourceError> {
        self.result
            .lock()
            .expect("mutex tidak diracuni")
            .take()
            .unwrap_or_else(|| Ok(FetchedComments::default()))
    }
}

/// Komentar valid siap pakai untuk test.
pub fn komentar(name: &str, status: &str) -> NewComment {
    NewComment::new(name, status, "Selamat ya!", None, None).expect("input valid")
}
