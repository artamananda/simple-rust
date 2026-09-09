//! Use case komentar: satu-satunya pintu masuk layer presentation ke domain.

use std::sync::Arc;

use uuid::Uuid;

use super::error::{ServiceError, ServiceResult};
use crate::domain::{
    Comment, CommentRepository, DomainError, ListCommentsQuery, NewComment, Page, UpdateComment,
};

#[derive(Clone)]
pub struct CommentService {
    repository: Arc<dyn CommentRepository>,
}

impl CommentService {
    pub fn new(repository: Arc<dyn CommentRepository>) -> Self {
        Self { repository }
    }

    pub async fn list(&self, query: ListCommentsQuery) -> ServiceResult<Page<Comment>> {
        Ok(self.repository.list(&query).await?)
    }

    pub async fn get(&self, id: Uuid) -> ServiceResult<Comment> {
        self.repository
            .find_by_id(id)
            .await?
            .ok_or(ServiceError::NotFound)
    }

    pub async fn create(&self, input: NewComment) -> ServiceResult<Comment> {
        Ok(self.repository.create(&input).await?)
    }

    pub async fn update(&self, id: Uuid, input: UpdateComment) -> ServiceResult<Comment> {
        if input.is_empty() {
            return Err(DomainError::validation("body", "minimal satu field harus diisi").into());
        }
        self.repository
            .update(id, &input)
            .await?
            .ok_or(ServiceError::NotFound)
    }

    pub async fn delete(&self, id: Uuid) -> ServiceResult<()> {
        if self.repository.delete(id).await? {
            Ok(())
        } else {
            Err(ServiceError::NotFound)
        }
    }
}
