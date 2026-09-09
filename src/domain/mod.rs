//! Layer domain: aturan bisnis murni.
//!
//! Tidak ada `axum`, `sqlx`, maupun `serde` di layer ini. Dependensi hanya
//! mengarah ke dalam (domain <- application <- infrastructure/presentation),
//! sehingga aturan bisnis bisa diuji tanpa database maupun HTTP server.

pub mod comment;
pub mod error;
pub mod pagination;
pub mod repository;

pub use comment::{
    Comment, CommentColor, CommentMessage, CommentName, CommentStatus, NewComment, UpdateComment,
};
pub use error::DomainError;
pub use pagination::{Page, Pagination};
pub use repository::{
    CommentRepository, ListCommentsQuery, RepositoryError, RepositoryResult, SortOrder,
};
