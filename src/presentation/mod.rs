//! Layer presentation: HTTP (Axum). Tugasnya hanya menerjemahkan request
//! menjadi input domain dan hasil use case menjadi JSON — tidak ada aturan
//! bisnis di sini.

pub mod dto;
pub mod error;
pub mod handlers;
pub mod response;
pub mod router;
pub mod state;

pub use error::ApiError;
pub use response::ApiResponse;
pub use router::build_router;
pub use state::AppState;
