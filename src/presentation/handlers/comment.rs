//! Handler CRUD komentar.
//!
//! Pola tiap handler sama: terjemahkan request -> panggil use case -> bungkus
//! hasilnya. Ekstraktor dibungkus `Result<_, Rejection>` supaya request yang
//! salah bentuk tetap dijawab dengan format JSON yang sama.

use axum::Json;
use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use uuid::Uuid;

use crate::presentation::dto::{
    CommentResponse, CreateCommentRequest, ListCommentsParams, SyncRequest, SyncResponse,
    UpdateCommentRequest,
};
use crate::presentation::error::ApiError;
use crate::presentation::response::{ApiResponse, PageMeta};
use crate::presentation::state::AppState;

type ApiResult<T> = Result<T, ApiError>;

/// `GET /api/comments`
pub async fn list(
    State(state): State<AppState>,
    params: Result<Query<ListCommentsParams>, QueryRejection>,
) -> ApiResult<ApiResponse<Vec<CommentResponse>>> {
    let Query(params) = params?;
    let page = state.comments.list(params.into_domain()?).await?;

    let meta = PageMeta::from_page(&page);
    let items = page.items.iter().map(CommentResponse::from).collect();

    Ok(ApiResponse::ok("Berhasil mengambil data", items).with_meta(meta))
}

/// `GET /api/comments/{id}`
pub async fn detail(
    State(state): State<AppState>,
    id: Result<Path<Uuid>, PathRejection>,
) -> ApiResult<ApiResponse<CommentResponse>> {
    let Path(id) = id?;
    let comment = state.comments.get(id).await?;

    Ok(ApiResponse::ok("Berhasil mengambil data", comment.into()))
}

/// `POST /api/comments`
pub async fn create(
    State(state): State<AppState>,
    payload: Result<Json<CreateCommentRequest>, JsonRejection>,
) -> ApiResult<ApiResponse<CommentResponse>> {
    let Json(payload) = payload?;
    let comment = state.comments.create(payload.into_domain()?).await?;

    Ok(ApiResponse::created(
        "Data berhasil ditambahkan",
        comment.into(),
    ))
}

/// `PUT /api/comments/{id}`
pub async fn update(
    State(state): State<AppState>,
    id: Result<Path<Uuid>, PathRejection>,
    payload: Result<Json<UpdateCommentRequest>, JsonRejection>,
) -> ApiResult<ApiResponse<CommentResponse>> {
    let Path(id) = id?;
    let Json(payload) = payload?;
    let comment = state.comments.update(id, payload.into_domain()?).await?;

    Ok(ApiResponse::ok("Data berhasil diperbarui", comment.into()))
}

/// `DELETE /api/comments/{id}`
pub async fn delete(
    State(state): State<AppState>,
    id: Result<Path<Uuid>, PathRejection>,
) -> ApiResult<ApiResponse<()>> {
    let Path(id) = id?;
    state.comments.delete(id).await?;

    Ok(ApiResponse::message("Data berhasil dihapus"))
}

/// `POST /api/comments/sync`
///
/// Menarik komentar dari sumber luar (endpoint Apps Script lama) lalu
/// menyimpannya dengan nama sebagai kunci — aman dijalankan berulang kali.
/// Dijaga kata kunci di body; nilainya diatur lewat `SYNC_SECRET`.
pub async fn sync(
    State(state): State<AppState>,
    payload: Result<Json<SyncRequest>, JsonRejection>,
) -> ApiResult<ApiResponse<SyncResponse>> {
    let Json(payload) = payload?;

    let Some(endpoint) = state.sync else {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "Sinkronisasi belum dikonfigurasi (isi SYNC_SOURCE_URL dan SYNC_SECRET)",
        ));
    };

    if payload.key() != &*endpoint.secret {
        // Alasan penolakan sengaja tidak dirinci.
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "Kata kunci sinkronisasi tidak sesuai",
        ));
    }

    let report = endpoint.service.run().await?;
    tracing::info!(
        fetched = report.fetched,
        created = report.created,
        updated = report.updated,
        skipped = report.skipped.len(),
        "sinkronisasi selesai"
    );

    Ok(ApiResponse::ok("Sinkronisasi selesai", report.into()))
}
