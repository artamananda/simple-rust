//! Endpoint kesehatan & metadata build — dipakai untuk memverifikasi hasil deploy.

use serde::Serialize;

use crate::build_info;
use crate::presentation::response::ApiResponse;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionResponse {
    pub service: &'static str,
    pub version: &'static str,
    pub commit: &'static str,
    pub build_time: &'static str,
}

impl VersionResponse {
    fn current() -> Self {
        Self {
            service: build_info::NAME,
            version: build_info::VERSION,
            commit: build_info::COMMIT,
            build_time: build_info::BUILD_TIME,
        }
    }
}

/// `GET /healthz`
pub async fn health() -> ApiResponse<VersionResponse> {
    ApiResponse::ok("Service berjalan normal", VersionResponse::current())
}

/// `GET /version`
pub async fn version() -> ApiResponse<VersionResponse> {
    ApiResponse::ok("Metadata build", VersionResponse::current())
}
