//! Test kontrak endpoint `/exec` — lapisan kompatibilitas Apps Script.
//!
//! Yang dikunci di sini persis hal-hal yang bikin frontend `our-wedding` patah
//! kalau berubah: bentuk JSON, urutan data, dan penerimaan body `text/plain`
//! (akibat `mode: 'no-cors'` di sisi browser).

mod common;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::{InMemoryCommentRepository, komentar};
use http_body_util::BodyExt;
use simple_rust::application::CommentService;
use simple_rust::config::HttpConfig;
use simple_rust::domain::NewComment;
use simple_rust::presentation::{AppState, build_router};
use tower::ServiceExt;

fn app(repository: Arc<InMemoryCommentRepository>) -> Router {
    let cfg = HttpConfig {
        addr: "127.0.0.1:0".to_owned(),
        allowed_origins: vec!["*".to_owned()],
        request_timeout: Duration::from_secs(5),
    };
    let state = AppState::new(
        Arc::new(CommentService::new(repository)),
        None,
        chrono::FixedOffset::east_opt(7 * 3600).expect("offset valid"),
    );

    build_router(state, &cfg)
}

async fn json(app: Router, request: Request<Body>) -> (StatusCode, serde_json::Value) {
    let response = app.oneshot(request).await.expect("request diproses");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body terbaca")
        .to_bytes();

    (status, serde_json::from_slice(&bytes).expect("body JSON"))
}

/// Persis payload yang dibentuk `wishas.js` saat form dikirim.
const PAYLOAD_FRONTEND: &str = concat!(
    r#"{"id":310773,"name":"Tamu Baru","status":"Hadir","#,
    r#""message":"Selamat ya!","date":"2026-09-09 23:42","color":"#,
    "\"#d8fe89\"}"
);

#[tokio::test]
async fn get_exec_membalas_bentuk_apps_script() {
    let repository = Arc::new(InMemoryCommentRepository::default());
    let service = CommentService::new(repository.clone());
    service
        .create(komentar("Arta", "Hadir"))
        .await
        .expect("tersimpan");

    let (status, body) = json(
        app(repository),
        Request::get("/exec").body(Body::empty()).expect("request"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], 200);
    assert_eq!(body["message"], "Berhasil mengambil data");

    // Kunci "comentar" (ejaan script lama) wajib ada — frontend melakukan
    // `const { comentar } = response`.
    let items = body["comentar"].as_array().expect("comentar berupa array");
    assert_eq!(items.len(), 1);

    let mut keys: Vec<&str> = items[0]
        .as_object()
        .expect("objek")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["color", "date", "id", "message", "name", "status"]);

    // Tidak boleh ada kunci tambahan di level atas.
    assert_eq!(body.as_object().expect("objek").len(), 3);
}

#[tokio::test]
async fn post_exec_menerima_content_type_text_plain() {
    // `mode: 'no-cors'` membuat browser MENURUNKAN Content-Type menjadi
    // text/plain. Ekstraktor Json bawaan Axum akan menolaknya dengan 415.
    let repository = Arc::new(InMemoryCommentRepository::default());

    let (status, body) = json(
        app(repository.clone()),
        Request::post("/exec")
            .header(header::CONTENT_TYPE, "text/plain;charset=UTF-8")
            .body(Body::from(PAYLOAD_FRONTEND))
            .expect("request"),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["status"], 200);
    assert_eq!(body["message"], "Data berhasil ditambahkan");

    let saved = CommentService::new(repository)
        .list_all()
        .await
        .expect("terbaca");
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].name.as_str(), "Tamu Baru");
    // 23:42 WIB disimpan sebagai 16:42 UTC.
    assert_eq!(saved[0].date.format("%H:%M").to_string(), "16:42");
}

#[tokio::test]
async fn post_exec_tetap_menerima_application_json() {
    let (status, body) = json(
        app(Arc::new(InMemoryCommentRepository::default())),
        Request::post("/exec")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(PAYLOAD_FRONTEND))
            .expect("request"),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "body: {body}");
    assert_eq!(body["message"], "Data berhasil ditambahkan");
}

#[tokio::test]
async fn get_exec_mengurutkan_terlama_lebih_dulu() {
    // Frontend memanggil comentar.reverse() sebelum menampilkan, jadi urutan
    // dari server harus menaik.
    let repository = Arc::new(InMemoryCommentRepository::default());
    let service = CommentService::new(repository.clone());

    for (name, waktu) in [
        ("Kedua", "2026-09-02T00:00:00Z"),
        ("Pertama", "2026-09-01T00:00:00Z"),
        ("Ketiga", "2026-09-03T00:00:00Z"),
    ] {
        let date = chrono::DateTime::parse_from_rfc3339(waktu)
            .expect("valid")
            .with_timezone(&chrono::Utc);
        service
            .create(NewComment::new(name, "Hadir", "halo", None, Some(date)).expect("valid"))
            .await
            .expect("tersimpan");
    }

    let (_, body) = json(
        app(repository),
        Request::get("/exec").body(Body::empty()).expect("request"),
    )
    .await;

    let names: Vec<&str> = body["comentar"]
        .as_array()
        .expect("array")
        .iter()
        .map(|item| item["name"].as_str().expect("nama"))
        .collect();
    assert_eq!(names, ["Pertama", "Kedua", "Ketiga"]);
}

#[tokio::test]
async fn post_exec_dengan_body_rusak_membalas_pesan_bergaya_lama() {
    let (status, body) = json(
        app(Arc::new(InMemoryCommentRepository::default())),
        Request::post("/exec")
            .header(header::CONTENT_TYPE, "text/plain")
            .body(Body::from("bukan json"))
            .expect("request"),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body["message"]
            .as_str()
            .expect("pesan")
            .starts_with("Kesalahan:"),
        "pesan error meniru gaya Apps Script, dapat: {body}"
    );
}
