//! Lapisan kompatibilitas dengan endpoint Apps Script lama.
//!
//! Frontend `our-wedding` memanggil satu URL untuk GET dan POST, dan bentuk
//! datanya sudah tertanam di sana. Modul ini menirunya persis supaya di sisi
//! frontend cukup mengganti `data.api` — tanpa menyentuh kode lain.
//!
//! Empat perbedaan yang harus ditutup (semuanya nyata di kode frontend):
//!
//! 1. GET membalas `{ status, message, comentar: [...] }` — bukan
//!    `{ status, message, data, meta }`, dan mengembalikan SEMUA baris
//!    sekaligus (frontend yang memotongnya jadi halaman 10-an).
//! 2. Urutan terlama lebih dulu, karena frontend memanggil `.reverse()`.
//! 3. POST dikirim dengan `mode: 'no-cors'`, sehingga browser MENURUNKAN
//!    `Content-Type` menjadi `text/plain`. Ekstraktor `Json` milik Axum akan
//!    menolaknya dengan 415, jadi body dibaca mentah lalu diurai sendiri —
//!    persis seperti `e.postData.contents` di Apps Script.
//! 4. `date` dikirim sebagai `"2026-09-09 23:42"` (waktu lokal, tanpa zona),
//!    bukan RFC 3339.

use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use chrono::{DateTime, FixedOffset, NaiveDateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::{Comment, DomainError, NewComment};
use crate::presentation::error::ApiError;
use crate::presentation::state::AppState;

/// Format tanggal yang dipakai Apps Script: ISO 8601 dengan milidetik + `Z`.
const DATE_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%.3fZ";

/// Format tanggal tanpa zona yang dikirim frontend lama.
const NAIVE_FORMATS: [&str; 3] = ["%Y-%m-%d %H:%M", "%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M"];

// ── Respons ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct LegacyListResponse {
    pub status: u16,
    pub message: String,
    /// Ejaan asli dari script lama dipertahankan — frontend mendestrukturisasi
    /// `const { comentar } = response`.
    pub comentar: Vec<LegacyComment>,
}

#[derive(Debug, Serialize)]
pub struct LegacyComment {
    pub id: String,
    pub name: String,
    pub status: String,
    pub message: String,
    pub date: String,
    pub color: String,
}

impl From<&Comment> for LegacyComment {
    fn from(comment: &Comment) -> Self {
        Self {
            id: comment.id.to_string(),
            name: comment.name.as_str().to_owned(),
            status: comment.status.as_str().to_owned(),
            message: comment.message.as_str().to_owned(),
            date: comment.date.format(DATE_FORMAT).to_string(),
            color: comment.color.as_str().to_owned(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct LegacyMessageResponse {
    pub status: u16,
    pub message: String,
}

// ── Handler ────────────────────────────────────────────────────────────────

/// `GET /exec` — padanan `doGet()` di Apps Script.
pub async fn list(State(state): State<AppState>) -> Result<Json<LegacyListResponse>, ApiError> {
    let comments = state.comments.list_all().await?;

    Ok(Json(LegacyListResponse {
        status: 200,
        message: "Berhasil mengambil data".to_owned(),
        comentar: comments.iter().map(LegacyComment::from).collect(),
    }))
}

/// `POST /exec` — padanan `doPost(e)` di Apps Script.
///
/// Body dibaca sebagai byte mentah, bukan lewat ekstraktor `Json`, supaya
/// request bertipe `text/plain` (akibat `mode: 'no-cors'`) tetap diterima.
pub async fn create(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<Json<LegacyMessageResponse>, ApiError> {
    let payload: LegacyCreateRequest = serde_json::from_slice(&body).map_err(|err| {
        ApiError::bad_request(format!("Kesalahan: body bukan JSON valid ({err})"))
    })?;

    let input = payload
        .into_domain(state.legacy_offset)
        .map_err(|err| ApiError::bad_request(format!("Kesalahan: {err}")))?;

    state.comments.create(input).await?;

    Ok(Json(LegacyMessageResponse {
        status: 200,
        message: "Data berhasil ditambahkan".to_owned(),
    }))
}

// ── Request ────────────────────────────────────────────────────────────────

/// Body POST versi lama. `id` diterima lalu diabaikan — id dibuat database.
#[derive(Debug, Default, Deserialize)]
pub struct LegacyCreateRequest {
    #[serde(default)]
    pub name: Option<Value>,
    #[serde(default)]
    pub status: Option<Value>,
    #[serde(default)]
    pub message: Option<Value>,
    #[serde(default)]
    pub color: Option<Value>,
    #[serde(default)]
    pub date: Option<Value>,
}

impl LegacyCreateRequest {
    pub fn into_domain(self, offset: FixedOffset) -> Result<NewComment, DomainError> {
        let date = match text(self.date.as_ref()) {
            Some(raw) => Some(parse_date(&raw, offset)?),
            None => None,
        };

        NewComment::new(
            &text(self.name.as_ref()).unwrap_or_default(),
            &text(self.status.as_ref()).unwrap_or_default(),
            &text(self.message.as_ref()).unwrap_or_default(),
            text(self.color.as_ref()).as_deref(),
            date,
        )
    }
}

/// Menerima RFC 3339 (ada zona waktunya) maupun format lokal tanpa zona.
fn parse_date(raw: &str, offset: FixedOffset) -> Result<DateTime<Utc>, DomainError> {
    if let Ok(date) = DateTime::parse_from_rfc3339(raw) {
        return Ok(date.with_timezone(&Utc));
    }

    for format in NAIVE_FORMATS {
        if let Ok(naive) = NaiveDateTime::parse_from_str(raw, format) {
            // Tanpa zona waktu, angka jam itu waktu lokal pengirim — bukan UTC.
            // Kalau ditafsirkan sebagai UTC, komentar tampil sebagai "7 jam
            // yang lalu" padahal baru saja dikirim.
            return offset
                .from_local_datetime(&naive)
                .single()
                .map(|date| date.with_timezone(&Utc))
                .ok_or_else(|| {
                    DomainError::validation("date", format!("'{raw}' ambigu di zona waktu ini"))
                });
        }
    }

    Err(DomainError::validation(
        "date",
        format!("'{raw}' bukan tanggal yang dikenali"),
    ))
}

/// Spreadsheet gampang mengubah tipe sel, jadi angka & boolean ikut diterima.
fn text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(value) => {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_owned())
        }
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wib() -> FixedOffset {
        FixedOffset::east_opt(7 * 3600).expect("offset valid")
    }

    #[test]
    fn tanggal_tanpa_zona_ditafsirkan_sebagai_waktu_lokal() {
        let date = parse_date("2026-09-09 23:42", wib()).expect("terbaca");

        // 23:42 WIB = 16:42 UTC pada hari yang sama.
        assert_eq!(
            date.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            "2026-09-09T16:42:00Z"
        );
    }

    #[test]
    fn tanggal_rfc3339_dipakai_apa_adanya() {
        let date = parse_date("2026-08-24T02:40:00.000Z", wib()).expect("terbaca");
        assert_eq!(
            date.format(DATE_FORMAT).to_string(),
            "2026-08-24T02:40:00.000Z"
        );
    }

    #[test]
    fn format_tanggal_keluaran_sama_dengan_apps_script() {
        let comment = Comment::from_parts(
            uuid::Uuid::nil(),
            "Arta",
            "Hadir",
            "Selamat!",
            "#048658",
            DateTime::parse_from_rfc3339("2026-08-24T02:40:00.123456Z")
                .expect("valid")
                .with_timezone(&Utc),
            Utc::now(),
            Utc::now(),
        )
        .expect("valid");

        let legacy = LegacyComment::from(&comment);
        // Milidetik + Z, persis seperti keluaran Apps Script.
        assert_eq!(legacy.date, "2026-08-24T02:40:00.123Z");
    }

    #[test]
    fn payload_frontend_lama_diterima_lengkap_dengan_id_yang_diabaikan() {
        let body = r##"{"id":310773,"name":"Arta","status":"Hadir",
                       "message":"Selamat ya!","date":"2026-09-09 23:42","color":"#d8fe89"}"##;
        let request: LegacyCreateRequest = serde_json::from_str(body).expect("terbaca");
        let input = request.into_domain(wib()).expect("valid");

        assert_eq!(input.name.as_str(), "Arta");
        assert_eq!(input.status.as_str(), "Hadir");
        assert_eq!(input.color.as_str(), "#d8fe89");
    }

    #[test]
    fn bentuk_respons_list_sama_dengan_apps_script() {
        let response = LegacyListResponse {
            status: 200,
            message: "Berhasil mengambil data".to_owned(),
            comentar: vec![],
        };
        let json = serde_json::to_value(&response).expect("serialisasi");

        // Kunci wajib persis: status, message, comentar.
        assert_eq!(json["status"], 200);
        assert_eq!(json["message"], "Berhasil mengambil data");
        assert!(json["comentar"].is_array());
        assert_eq!(json.as_object().expect("objek").len(), 3);
    }
}
