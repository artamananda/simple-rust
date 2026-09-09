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
    RepositoryResult, SortOrder, SyncStats, UpdateComment,
};

/// Kunci pengunci antar-transaksi untuk endpoint sync ("SYNC" dalam heksa).
const SYNC_LOCK_KEY: i64 = 0x5359_4E43;

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

    async fn list_all(&self) -> RepositoryResult<Vec<Comment>> {
        // Urutan menaik meniru urutan baris di spreadsheet: frontend lama
        // memanggil .reverse() untuk menampilkan yang terbaru lebih dulu.
        let rows: Vec<CommentRow> = sqlx::query_as(&format!(
            "SELECT {COLUMNS} FROM comments ORDER BY commented_at ASC, created_at ASC"
        ))
        .fetch_all(&self.pool)
        .await
        .map_err(backend)?;

        Ok(rows
            .into_iter()
            .map(Comment::try_from)
            .collect::<Result<Vec<_>, _>>()?)
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

    async fn upsert_many_by_name(&self, inputs: &[NewComment]) -> RepositoryResult<SyncStats> {
        // Seluruh sinkronisasi berjalan dalam satu transaksi: kalau ada baris
        // yang gagal, tidak ada yang setengah tersimpan.
        let mut tx = self.pool.begin().await.map_err(backend)?;

        // Kunci tingkat transaksi membuat dua sync yang berjalan bersamaan
        // antre, bukan sama-sama menyisipkan nama yang sama.
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(SYNC_LOCK_KEY)
            .execute(&mut *tx)
            .await
            .map_err(backend)?;

        let mut stats = SyncStats::default();

        for input in inputs {
            // Pencocokan mengabaikan huruf besar/kecil dan spasi di ujung —
            // "Budi " dari sheet dianggap orang yang sama dengan "budi".
            let existing: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM comments
                  WHERE lower(btrim(name)) = lower(btrim($1))
                  ORDER BY created_at
                  LIMIT 1",
            )
            .bind(input.name.as_str())
            .fetch_optional(&mut *tx)
            .await
            .map_err(backend)?;

            match existing {
                // Nama ikut ditulis ulang supaya ejaan terbaru dari sumber menang.
                Some(id) => {
                    sqlx::query(
                        "UPDATE comments
                            SET name         = $2,
                                status       = $3,
                                message      = $4,
                                color        = $5,
                                commented_at = $6,
                                updated_at   = NOW()
                          WHERE id = $1",
                    )
                    .bind(id)
                    .bind(input.name.as_str())
                    .bind(input.status.as_str())
                    .bind(input.message.as_str())
                    .bind(input.color.as_str())
                    .bind(input.date)
                    .execute(&mut *tx)
                    .await
                    .map_err(backend)?;

                    stats.updated += 1;
                }
                None => {
                    sqlx::query(
                        "INSERT INTO comments (name, status, message, color, commented_at)
                         VALUES ($1, $2, $3, $4, $5)",
                    )
                    .bind(input.name.as_str())
                    .bind(input.status.as_str())
                    .bind(input.message.as_str())
                    .bind(input.color.as_str())
                    .bind(input.date)
                    .execute(&mut *tx)
                    .await
                    .map_err(backend)?;

                    stats.created += 1;
                }
            }
        }

        tx.commit().await.map_err(backend)?;

        Ok(stats)
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
