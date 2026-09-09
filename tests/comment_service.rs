//! Test use case tanpa database.
//!
//! `CommentService` hanya bergantung pada trait `CommentRepository`, jadi di
//! sini trait itu diisi implementasi in-memory. Inilah keuntungan konkret dari
//! pemisahan layer: aturan bisnis bisa diuji tanpa Postgres, tanpa HTTP, dan
//! selesai dalam hitungan milidetik.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use simple_rust::application::{CommentService, ServiceError, SyncService};
use simple_rust::domain::{
    Comment, CommentRepository, CommentSource, FetchedComments, ListCommentsQuery, NewComment,
    Page, Pagination, RepositoryResult, SkippedComment, SortOrder, SourceError, SyncStats,
    UpdateComment,
};
use uuid::Uuid;

#[derive(Default)]
struct InMemoryCommentRepository {
    items: Mutex<Vec<Comment>>,
}

impl InMemoryCommentRepository {
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Comment>> {
        self.items.lock().expect("mutex tidak diracuni")
    }
}

#[async_trait]
impl CommentRepository for InMemoryCommentRepository {
    async fn list(&self, query: &ListCommentsQuery) -> RepositoryResult<Page<Comment>> {
        let mut items: Vec<Comment> = self
            .lock()
            .iter()
            .filter(|comment| match &query.status {
                Some(status) => comment.status.as_str() == status,
                None => true,
            })
            .cloned()
            .collect();

        match query.sort {
            SortOrder::Newest => items.sort_by_key(|item| std::cmp::Reverse(item.date)),
            SortOrder::Oldest => items.sort_by_key(|item| item.date),
        }

        let total = items.len() as i64;
        let page: Vec<Comment> = items
            .into_iter()
            .skip(query.pagination.offset() as usize)
            .take(query.pagination.limit() as usize)
            .collect();

        Ok(Page::new(page, total, query.pagination))
    }

    async fn find_by_id(&self, id: Uuid) -> RepositoryResult<Option<Comment>> {
        Ok(self.lock().iter().find(|comment| comment.id == id).cloned())
    }

    async fn create(&self, input: &NewComment) -> RepositoryResult<Comment> {
        let now = Utc::now();
        let comment = Comment {
            id: Uuid::new_v4(),
            name: input.name.clone(),
            status: input.status.clone(),
            message: input.message.clone(),
            color: input.color.clone(),
            date: input.date,
            created_at: now,
            updated_at: now,
        };

        self.lock().push(comment.clone());
        Ok(comment)
    }

    async fn update(&self, id: Uuid, input: &UpdateComment) -> RepositoryResult<Option<Comment>> {
        let mut items = self.lock();
        let Some(comment) = items.iter_mut().find(|comment| comment.id == id) else {
            return Ok(None);
        };

        if let Some(name) = input.name.clone() {
            comment.name = name;
        }
        if let Some(status) = input.status.clone() {
            comment.status = status;
        }
        if let Some(message) = input.message.clone() {
            comment.message = message;
        }
        if let Some(color) = input.color.clone() {
            comment.color = color;
        }
        if let Some(date) = input.date {
            comment.date = date;
        }
        comment.updated_at = Utc::now();

        Ok(Some(comment.clone()))
    }

    async fn delete(&self, id: Uuid) -> RepositoryResult<bool> {
        let mut items = self.lock();
        let before = items.len();
        items.retain(|comment| comment.id != id);

        Ok(items.len() != before)
    }

    async fn upsert_many_by_name(&self, inputs: &[NewComment]) -> RepositoryResult<SyncStats> {
        let mut stats = SyncStats::default();

        for input in inputs {
            let key = input.name.as_str().trim().to_lowercase();
            let mut items = self.lock();

            match items
                .iter_mut()
                .find(|comment| comment.name.as_str().trim().to_lowercase() == key)
            {
                Some(existing) => {
                    existing.name = input.name.clone();
                    existing.status = input.status.clone();
                    existing.message = input.message.clone();
                    existing.color = input.color.clone();
                    existing.date = input.date;
                    existing.updated_at = Utc::now();
                    stats.updated += 1;
                }
                None => {
                    let now = Utc::now();
                    items.push(Comment {
                        id: Uuid::new_v4(),
                        name: input.name.clone(),
                        status: input.status.clone(),
                        message: input.message.clone(),
                        color: input.color.clone(),
                        date: input.date,
                        created_at: now,
                        updated_at: now,
                    });
                    stats.created += 1;
                }
            }
        }

        Ok(stats)
    }
}

/// Sumber palsu: mengembalikan apa yang disiapkan test, tanpa menyentuh jaringan.
struct FakeSource {
    result: Mutex<Option<Result<FetchedComments, SourceError>>>,
}

impl FakeSource {
    fn returning(comments: Vec<NewComment>, skipped: Vec<SkippedComment>) -> Self {
        Self {
            result: Mutex::new(Some(Ok(FetchedComments { comments, skipped }))),
        }
    }

    fn failing() -> Self {
        Self {
            result: Mutex::new(Some(Err(SourceError::Unreachable(anyhow::anyhow!(
                "koneksi timeout"
            ))))),
        }
    }
}

#[async_trait]
impl CommentSource for FakeSource {
    async fn fetch_all(&self) -> Result<FetchedComments, SourceError> {
        self.result
            .lock()
            .expect("mutex tidak diracuni")
            .take()
            .unwrap_or_else(|| Ok(FetchedComments::default()))
    }
}

fn service() -> CommentService {
    CommentService::new(Arc::new(InMemoryCommentRepository::default()))
}

fn komentar(name: &str, status: &str) -> NewComment {
    NewComment::new(name, status, "Selamat ya!", None, None).expect("input valid")
}

#[tokio::test]
async fn komentar_yang_dibuat_bisa_dibaca_lagi() {
    let service = service();

    let created = service
        .create(komentar("Arta", "hadir"))
        .await
        .expect("tersimpan");
    let found = service.get(created.id).await.expect("ketemu");

    assert_eq!(found.id, created.id);
    assert_eq!(found.name.as_str(), "Arta");
    assert_eq!(found.color.as_str(), "#000000", "warna default dipakai");
}

#[tokio::test]
async fn list_memfilter_status_dan_menghormati_paginasi() {
    let service = service();
    for index in 0..3 {
        service
            .create(komentar(&format!("Tamu {index}"), "hadir"))
            .await
            .expect("tersimpan");
    }
    service
        .create(komentar("Tamu absen", "tidak hadir"))
        .await
        .expect("tersimpan");

    let query = ListCommentsQuery {
        pagination: Pagination::new(1, 2).expect("valid"),
        status: Some("hadir".to_owned()),
        ..Default::default()
    };
    let page = service.list(query).await.expect("berhasil");

    assert_eq!(page.items.len(), 2, "dibatasi perPage");
    assert_eq!(page.total, 3, "total menghitung seluruh baris yang cocok");
    assert_eq!(page.total_pages(), 2);
}

#[tokio::test]
async fn update_hanya_mengubah_field_yang_dikirim() {
    let service = service();
    let created = service
        .create(komentar("Arta", "hadir"))
        .await
        .expect("tersimpan");

    let patch = UpdateComment::new(None, Some("tidak hadir"), None, None, None).expect("valid");
    let updated = service.update(created.id, patch).await.expect("terupdate");

    assert_eq!(updated.status.as_str(), "tidak hadir");
    assert_eq!(updated.name.as_str(), "Arta", "field lain tidak tersentuh");
    assert_eq!(updated.message.as_str(), created.message.as_str());
}

#[tokio::test]
async fn update_tanpa_field_ditolak() {
    let service = service();
    let created = service
        .create(komentar("Arta", "hadir"))
        .await
        .expect("tersimpan");

    let err = service
        .update(created.id, UpdateComment::default())
        .await
        .expect_err("harus ditolak");

    assert!(matches!(err, ServiceError::Validation(_)), "dapat: {err:?}");
}

#[tokio::test]
async fn operasi_pada_id_tak_dikenal_menghasilkan_not_found() {
    let service = service();
    let id = Uuid::new_v4();

    assert!(matches!(
        service.get(id).await.expect_err("tidak ada"),
        ServiceError::NotFound
    ));
    assert!(matches!(
        service.delete(id).await.expect_err("tidak ada"),
        ServiceError::NotFound
    ));
}

#[tokio::test]
async fn delete_menghapus_komentar() {
    let service = service();
    let created = service
        .create(komentar("Arta", "hadir"))
        .await
        .expect("tersimpan");

    service.delete(created.id).await.expect("terhapus");

    assert!(matches!(
        service.get(created.id).await.expect_err("sudah hilang"),
        ServiceError::NotFound
    ));
}

#[tokio::test]
async fn input_tidak_valid_ditolak_sebelum_menyentuh_repository() {
    let err = NewComment::new("", "hadir", "halo", None, None).expect_err("nama kosong");
    assert!(err.to_string().contains("name"));

    let err = NewComment::new("Arta", "hadir", "halo", Some("bukan-warna!!"), None)
        .expect_err("warna ngawur");
    assert!(err.to_string().contains("color"));
}

// ── Sinkronisasi ────────────────────────────────────────────────────────────

#[tokio::test]
async fn sync_menambahkan_data_baru_lalu_memperbaruinya_saat_diulang() {
    let repository = Arc::new(InMemoryCommentRepository::default());

    let batch = || vec![komentar("Arta", "Hadir"), komentar("Budi", "Tidak Hadir")];

    // Jalan pertama: dua-duanya baru.
    let first = SyncService::new(
        Arc::new(FakeSource::returning(batch(), vec![])),
        repository.clone(),
    )
    .run()
    .await
    .expect("sync berhasil");
    assert_eq!((first.created, first.updated), (2, 0));

    // Jalan kedua dengan data sama: tidak ada yang digandakan.
    let second = SyncService::new(
        Arc::new(FakeSource::returning(batch(), vec![])),
        repository.clone(),
    )
    .run()
    .await
    .expect("sync berhasil");
    assert_eq!(
        (second.created, second.updated),
        (0, 2),
        "sync harus idempoten"
    );

    let all = CommentService::new(repository)
        .list(ListCommentsQuery::default())
        .await
        .expect("berhasil");
    assert_eq!(all.total, 2, "tidak ada duplikat setelah sync berulang");
}

#[tokio::test]
async fn sync_mencocokkan_nama_tanpa_peduli_huruf_besar_kecil_dan_spasi() {
    let repository = Arc::new(InMemoryCommentRepository::default());

    SyncService::new(
        Arc::new(FakeSource::returning(
            vec![komentar("Arta", "Hadir")],
            vec![],
        )),
        repository.clone(),
    )
    .run()
    .await
    .expect("sync pertama");

    let report = SyncService::new(
        Arc::new(FakeSource::returning(
            vec![NewComment::new("  aRtA  ", "Tidak Hadir", "berubah", None, None).expect("valid")],
            vec![],
        )),
        repository.clone(),
    )
    .run()
    .await
    .expect("sync kedua");

    assert_eq!((report.created, report.updated), (0, 1));

    let all = CommentService::new(repository)
        .list(ListCommentsQuery::default())
        .await
        .expect("berhasil");
    assert_eq!(all.total, 1);
    assert_eq!(
        all.items[0].name.as_str(),
        "aRtA",
        "ejaan terbaru dari sumber menang"
    );
    assert_eq!(all.items[0].status.as_str(), "Tidak Hadir");
}

#[tokio::test]
async fn sync_melaporkan_baris_yang_dilewati_tanpa_menggagalkan_sisanya() {
    let repository = Arc::new(InMemoryCommentRepository::default());
    let skipped = vec![SkippedComment {
        name: "Tanggal Rusak".to_owned(),
        reason: "date: bukan tanggal RFC 3339".to_owned(),
    }];

    let report = SyncService::new(
        Arc::new(FakeSource::returning(
            vec![komentar("Arta", "Hadir")],
            skipped,
        )),
        repository,
    )
    .run()
    .await
    .expect("sync berhasil");

    assert_eq!(
        report.fetched, 2,
        "yang dilewati tetap dihitung sebagai diterima"
    );
    assert_eq!(report.created, 1);
    assert_eq!(report.skipped.len(), 1);
    assert_eq!(report.skipped[0].name, "Tanggal Rusak");
}

#[tokio::test]
async fn sumber_bermasalah_dilaporkan_sebagai_upstream() {
    let service = SyncService::new(
        Arc::new(FakeSource::failing()),
        Arc::new(InMemoryCommentRepository::default()),
    );

    let err = service.run().await.expect_err("sumber mati");
    assert!(matches!(err, ServiceError::Upstream(_)), "dapat: {err:?}");
}
