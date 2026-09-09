//! Error use case: satu tipe yang cukup untuk dipetakan langsung ke status HTTP.

use thiserror::Error;

use crate::domain::{DomainError, RepositoryError};

pub type ServiceResult<T> = Result<T, ServiceError>;

#[derive(Debug, Error)]
pub enum ServiceError {
    /// Input pemanggil melanggar aturan domain -> 400.
    #[error(transparent)]
    Validation(#[from] DomainError),

    /// Resource tidak ada -> 404.
    #[error("komentar tidak ditemukan")]
    NotFound,

    /// Kegagalan teknis -> 500 (detailnya dicatat di log, bukan dikirim ke klien).
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

impl From<RepositoryError> for ServiceError {
    fn from(err: RepositoryError) -> Self {
        match err {
            RepositoryError::Corrupt(err) => {
                Self::Unexpected(anyhow::anyhow!("data tersimpan tidak valid: {err}"))
            }
            RepositoryError::Backend(err) => Self::Unexpected(err),
        }
    }
}
