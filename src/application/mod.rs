//! Layer application: use case yang mengorkestrasi domain.
//!
//! Bergantung pada *trait* repository, bukan implementasinya — jadi service di
//! sini bisa diuji dengan repository in-memory tanpa menyalakan Postgres
//! (lihat `tests/comment_service.rs`).

pub mod comment_service;
pub mod error;

pub use comment_service::CommentService;
pub use error::{ServiceError, ServiceResult};
