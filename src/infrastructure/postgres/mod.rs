//! Implementasi repository di atas Postgres (sqlx).

pub mod comment_repository;

pub use comment_repository::PgCommentRepository;
