//! Test use case tanpa database.
//!
//! `CommentService` hanya bergantung pada trait `CommentRepository`, jadi di
//! sini trait itu diisi implementasi in-memory. Inilah keuntungan konkret dari
//! pemisahan layer: aturan bisnis bisa diuji tanpa Postgres, tanpa HTTP, dan
//! selesai dalam hitungan milidetik.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use simple_rust::application::{CommentService, ServiceError};
use simple_rust::domain::{
    Comment, CommentRepository, ListCommentsQuery, NewComment, Page, Pagination, RepositoryResult,
    SortOrder, UpdateComment,
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
