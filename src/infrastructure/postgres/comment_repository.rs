//! `CommentRepository` versi Postgres.
//!
//! Baris database dibaca ke `CommentRow` lalu diubah menjadi entity domain.
//! Pemisahan ini menjaga entity tetap bersih dari atribut `sqlx`, sekaligus
//! menjadi tempat pemetaan kolom `commented_at` <-> field domain `date`.

use anyhow::Error as AnyError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::{
    Comment, CommentRepository, DomainError, ListCommentsQuery, NewComment, Page, RepositoryError,
    RepositoryResult, SortOrder, UpdateComment,
};

const COLUMNS: &str = "id, name, status, message, color, commented_at, created_at, updated_at";

/// Filter yang dipakai bersama oleh query hitung total dan query ambil data.
/// `$1` = status, `$2` = kata kunci pencarian.
const FILTER: &str = "WHERE ($1::text IS NULL OR status = $1)
       AND ($2::text IS NULL
            OR name ILIKE '%' || $2 || '%'
            OR message ILIKE '%' || $2 || '%')";

pub struct PgCommentRepository {
    pool: PgPool,
}

impl PgCommentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CommentRepository for PgCommentRepository {
    async fn list(&self, query: &ListCommentsQuery) -> RepositoryResult<Page<Comment>> {
        let status = query.status.as_deref();
        let search = query.search.as_deref();

        let total: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM comments {FILTER}"))
            .bind(status)
            .bind(search)
            .fetch_one(&self.pool)
            .await
            .map_err(backend)?;

        // Klausa ORDER BY dipilih dari konstanta lewat `match`, bukan dirangkai
        // dari input pengguna — jadi tidak ada celah SQL injection di sini.
        let order = match query.sort {
            SortOrder::Newest => "ORDER BY commented_at DESC, created_at DESC",
            SortOrder::Oldest => "ORDER BY commented_at ASC, created_at ASC",
        };

        let rows: Vec<CommentRow> = sqlx::query_as(&format!(
            "SELECT {COLUMNS} FROM comments {FILTER} {order} LIMIT $3 OFFSET $4"
        ))
        .bind(status)
        .bind(search)
        .bind(query.pagination.limit())
        .bind(query.pagination.offset())
        .fetch_all(&self.pool)
        .await
        .map_err(backend)?;

        let items = rows
            .into_iter()
            .map(Comment::try_from)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Page::new(items, total, query.pagination))
    }

    async fn find_by_id(&self, id: Uuid) -> RepositoryResult<Option<Comment>> {
        let row: Option<CommentRow> =
            sqlx::query_as(&format!("SELECT {COLUMNS} FROM comments WHERE id = $1"))
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(backend)?;

        Ok(row.map(Comment::try_from).transpose()?)
    }

    async fn create(&self, input: &NewComment) -> RepositoryResult<Comment> {
        let row: CommentRow = sqlx::query_as(&format!(
            "INSERT INTO comments (name, status, message, color, commented_at)
             VALUES ($1, $2, $3, $4, $5)
             RETURNING {COLUMNS}"
        ))
        .bind(input.name.as_str())
        .bind(input.status.as_str())
        .bind(input.message.as_str())
        .bind(input.color.as_str())
        .bind(input.date)
        .fetch_one(&self.pool)
        .await
        .map_err(backend)?;

        Ok(Comment::try_from(row)?)
    }

    async fn update(&self, id: Uuid, input: &UpdateComment) -> RepositoryResult<Option<Comment>> {
        // COALESCE: parameter NULL berarti "biarkan nilai lama".
        let row: Option<CommentRow> = sqlx::query_as(&format!(
            "UPDATE comments
                SET name         = COALESCE($2::text, name),
                    status       = COALESCE($3::text, status),
                    message      = COALESCE($4::text, message),
                    color        = COALESCE($5::text, color),
                    commented_at = COALESCE($6::timestamptz, commented_at),
                    updated_at   = NOW()
              WHERE id = $1
              RETURNING {COLUMNS}"
        ))
        .bind(id)
        .bind(input.name.as_ref().map(|value| value.as_str()))
        .bind(input.status.as_ref().map(|value| value.as_str()))
        .bind(input.message.as_ref().map(|value| value.as_str()))
        .bind(input.color.as_ref().map(|value| value.as_str()))
        .bind(input.date)
        .fetch_optional(&self.pool)
        .await
        .map_err(backend)?;

        Ok(row.map(Comment::try_from).transpose()?)
    }

    async fn delete(&self, id: Uuid) -> RepositoryResult<bool> {
        let result = sqlx::query("DELETE FROM comments WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(backend)?;

        Ok(result.rows_affected() > 0)
    }
}

/// Representasi satu baris tabel `comments`.
#[derive(Debug, sqlx::FromRow)]
struct CommentRow {
    id: Uuid,
    name: String,
    status: String,
    message: String,
    color: String,
    commented_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<CommentRow> for Comment {
    type Error = DomainError;

    fn try_from(row: CommentRow) -> Result<Self, Self::Error> {
        Comment::from_parts(
            row.id,
            &row.name,
            &row.status,
            &row.message,
            &row.color,
            row.commented_at,
            row.created_at,
            row.updated_at,
        )
    }
}

fn backend(err: sqlx::Error) -> RepositoryError {
    RepositoryError::Backend(AnyError::new(err))
}
