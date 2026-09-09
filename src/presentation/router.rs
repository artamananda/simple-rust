//! Susunan rute dan middleware global.

use std::time::Duration;

use axum::Router;
use axum::http::{HeaderValue, Method, StatusCode, Uri, header};
use axum::routing::get;
use tower_http::cors::{Any, CorsLayer};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::config::HttpConfig;
use crate::presentation::error::ApiError;
use crate::presentation::handlers::{comment, health};
use crate::presentation::state::AppState;

pub fn build_router(state: AppState, cfg: &HttpConfig) -> Router {
    let comments = Router::new()
        .route("/comments", get(comment::list).post(comment::create))
        .route(
            "/comments/{id}",
            get(comment::detail)
                .put(comment::update)
                .delete(comment::delete),
        );

    Router::new()
        .route("/healthz", get(health::health))
        .route("/version", get(health::version))
        .nest("/api", comments)
        .fallback(not_found)
        .layer(TraceLayer::new_for_http())
        .layer(cors_layer(&cfg.allowed_origins))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            cfg.request_timeout,
        ))
        .with_state(state)
}

/// `WEB_ORIGIN` boleh berisi beberapa origin dipisah koma, atau `*` untuk
/// mengizinkan semuanya (default, meniru perilaku endpoint Apps Script).
fn cors_layer(allowed_origins: &[String]) -> CorsLayer {
    let layer = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
        .max_age(Duration::from_secs(3_600));

    if allowed_origins.iter().any(|origin| origin == "*") {
        return layer.allow_origin(Any);
    }

    let origins: Vec<HeaderValue> = allowed_origins
        .iter()
        .filter_map(|origin| match origin.parse::<HeaderValue>() {
            Ok(value) => Some(value),
            Err(_) => {
                tracing::warn!(origin, "WEB_ORIGIN diabaikan karena tidak valid");
                None
            }
        })
        .collect();

    layer.allow_origin(origins)
}

async fn not_found(uri: Uri) -> ApiError {
    ApiError::not_found(format!("Rute {uri} tidak ditemukan"))
}
