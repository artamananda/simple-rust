//! Bentuk respons sukses yang seragam:
//!
//! ```json
//! { "status": 200, "message": "...", "data": ..., "meta": { ... } }
//! ```

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use crate::domain::Page;

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub status: u16,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<PageMeta>,
}

impl<T> ApiResponse<T> {
    pub fn ok(message: impl Into<String>, data: T) -> Self {
        Self::new(StatusCode::OK, message, Some(data))
    }

    pub fn created(message: impl Into<String>, data: T) -> Self {
        Self::new(StatusCode::CREATED, message, Some(data))
    }

    pub fn with_meta(mut self, meta: PageMeta) -> Self {
        self.meta = Some(meta);
        self
    }

    fn new(status: StatusCode, message: impl Into<String>, data: Option<T>) -> Self {
        Self {
            status: status.as_u16(),
            message: message.into(),
            data,
            meta: None,
        }
    }
}

impl ApiResponse<()> {
    /// Respons tanpa payload (mis. setelah menghapus data).
    pub fn message(message: impl Into<String>) -> Self {
        Self::new(StatusCode::OK, message, None)
    }
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.status).unwrap_or(StatusCode::OK);
        (status, Json(self)).into_response()
    }
}

/// Metadata paginasi untuk endpoint list.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageMeta {
    pub page: u32,
    pub per_page: u32,
    pub total: i64,
    pub total_pages: i64,
}

impl PageMeta {
    pub fn from_page<T>(page: &Page<T>) -> Self {
        Self {
            page: page.pagination.page(),
            per_page: page.pagination.per_page(),
            total: page.total,
            total_pages: page.total_pages(),
        }
    }
}
