//! Error yang lahir dari aturan bisnis, bukan dari infrastruktur.

use thiserror::Error;

/// Pelanggaran aturan domain. Selalu bisa diperbaiki oleh pemanggil, jadi di
/// layer HTTP dipetakan menjadi 400 Bad Request.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DomainError {
    #[error("{field}: {message}")]
    Validation {
        field: &'static str,
        message: String,
    },
}

impl DomainError {
    pub fn validation(field: &'static str, message: impl Into<String>) -> Self {
        Self::Validation {
            field,
            message: message.into(),
        }
    }
}
