//! Pemetaan error ke respons HTTP. Bentuk body-nya sama dengan respons sukses
//! (`status` + `message`) supaya klien cukup menangani satu format.

use axum::Json;
use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::application::ServiceError;
use crate::domain::DomainError;

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(json!({
            "status": self.status.as_u16(),
            "message": self.message,
        }));
        (self.status, body).into_response()
    }
}

impl From<ServiceError> for ApiError {
    fn from(err: ServiceError) -> Self {
        match err {
            ServiceError::Validation(err) => Self::bad_request(err.to_string()),
            ServiceError::NotFound => Self::not_found("Komentar tidak ditemukan"),
            ServiceError::Unexpected(err) => {
                // Detail teknis hanya untuk log; klien cukup tahu ada kegagalan server.
                tracing::error!(error = ?err, "kesalahan tak terduga");
                Self::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Terjadi kesalahan pada server",
                )
            }
        }
    }
}

impl From<DomainError> for ApiError {
    fn from(err: DomainError) -> Self {
        Self::bad_request(err.to_string())
    }
}

impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        Self::bad_request(format!("Body JSON tidak valid: {}", rejection.body_text()))
    }
}

impl From<QueryRejection> for ApiError {
    fn from(rejection: QueryRejection) -> Self {
        Self::bad_request(format!(
            "Query string tidak valid: {}",
            rejection.body_text()
        ))
    }
}

impl From<PathRejection> for ApiError {
    fn from(rejection: PathRejection) -> Self {
        Self::bad_request(format!(
            "Parameter URL tidak valid: {}",
            rejection.body_text()
        ))
    }
}
