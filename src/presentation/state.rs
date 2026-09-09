//! State yang dibagikan ke seluruh handler.

use std::sync::Arc;

use crate::application::CommentService;

#[derive(Clone)]
pub struct AppState {
    pub comments: Arc<CommentService>,
}

impl AppState {
    pub fn new(comments: Arc<CommentService>) -> Self {
        Self { comments }
    }
}
