//! Sumber komentar dari endpoint Apps Script lama.
//!
//! Bertindak sebagai *anti-corruption layer*: bentuk respons Apps Script
//! (`{ status, message, comentar: [...] }`, `id` berupa angka, tanggal string)
//! diterjemahkan ke tipe domain di sini, sehingga layer lain tidak perlu tahu
//! bahwa data ini pernah tinggal di Google Spreadsheet.
//!
//! Pembacaannya sengaja longgar: satu baris cacat tidak boleh menggagalkan
//! seluruh sinkronisasi — baris itu dilewati dan alasannya dilaporkan.

use std::time::Duration;

use anyhow::{Context, anyhow};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;

use crate::domain::{CommentSource, FetchedComments, NewComment, SkippedComment, SourceError};

pub struct AppsScriptCommentSource {
    client: reqwest::Client,
    url: String,
}

impl AppsScriptCommentSource {
    pub fn new(url: impl Into<String>, timeout: Duration) -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            // Apps Script menjawab lewat beberapa kali redirect ke googleusercontent.
            .redirect(reqwest::redirect::Policy::limited(10))
            .user_agent(concat!("simple-rust/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("gagal menyiapkan HTTP client")?;

        Ok(Self {
            client,
            url: url.into(),
        })
    }
}

#[async_trait]
impl CommentSource for AppsScriptCommentSource {
    async fn fetch_all(&self) -> Result<FetchedComments, SourceError> {
        let response = self
            .client
            .get(&self.url)
            .send()
            .await
            .map_err(|err| SourceError::Unreachable(anyhow!(err)))?;

        let status = response.status();
        if !status.is_success() {
            return Err(SourceError::Unreachable(anyhow!(
                "sumber menjawab dengan status {status}"
            )));
        }

        let body = response
            .text()
            .await
            .map_err(|err| SourceError::Unreachable(anyhow!(err)))?;

        parse_payload(&body)
    }
}

/// Bentuk respons Apps Script. Field `comentar` (ejaan asli di script lama)
/// dipertahankan di sini — hanya di titik ini nama itu masih hidup.
#[derive(Debug, Deserialize)]
struct AppsScriptPayload {
    #[serde(default)]
    comentar: Vec<Value>,
}

fn parse_payload(body: &str) -> Result<FetchedComments, SourceError> {
    let payload: AppsScriptPayload = serde_json::from_str(body).map_err(|err| {
        SourceError::InvalidResponse(anyhow!(
            "gagal membaca JSON dari sumber: {err} (potongan: {})",
            body.chars().take(120).collect::<String>()
        ))
    })?;

    let mut result = FetchedComments::default();

    for row in &payload.comentar {
        let name = text(row.get("name")).unwrap_or_default();

        match to_new_comment(row) {
            Ok(comment) => result.comments.push(comment),
            Err(reason) => result.skipped.push(SkippedComment { name, reason }),
        }
    }

    Ok(result)
}

fn to_new_comment(row: &Value) -> Result<NewComment, String> {
    let name = text(row.get("name")).unwrap_or_default();
    let status = text(row.get("status")).unwrap_or_default();
    let message = text(row.get("message")).unwrap_or_default();
    let color = text(row.get("color"));

    // Tanggal diurai terpisah supaya format yang tak terbaca melewatkan satu
    // baris saja, bukan menggagalkan seluruh payload.
    let date = match text(row.get("date")) {
        Some(raw) => Some(parse_date(&raw).map_err(|err| format!("date: {err}"))?),
        None => None,
    };

    NewComment::new(&name, &status, &message, color.as_deref(), date).map_err(|err| err.to_string())
}

/// Spreadsheet gampang mengubah tipe sel (nama berupa angka, misalnya), jadi
/// angka dan boolean ikut diterima lalu dijadikan teks.
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

fn parse_date(raw: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(raw)
        .map(|date| date.with_timezone(&Utc))
        .map_err(|err| format!("'{raw}' bukan tanggal RFC 3339 ({err})"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAYLOAD: &str = r##"{
        "status": 200,
        "message": "Berhasil mengambil data",
        "comentar": [
            {"id": 785711, "name": "Achmad Mario", "status": "Hadir",
             "message": "Selamat ya!", "date": "2026-08-24T02:40:00.000Z", "color": "#048658"},
            {"id": 226012, "name": "  Nama Berspasi  ", "status": "Tidak Hadir",
             "message": "Maaf berhalangan", "date": "2026-09-06T07:06:00.000Z", "color": "#0bb39d"},
            {"id": 1, "name": "", "status": "Hadir",
             "message": "tanpa nama", "date": "2026-09-06T07:06:00.000Z", "color": "#000000"},
            {"id": 2, "name": "Tanggal Rusak", "status": "Hadir",
             "message": "halo", "date": "bukan tanggal", "color": "#000000"},
            {"id": 3, "name": 12345, "status": "Hadir",
             "message": "nama berupa angka", "date": "2026-09-06T07:06:00.000Z", "color": "#fff"}
        ]
    }"##;

    #[test]
    fn baris_valid_diterjemahkan_ke_domain() {
        let fetched = parse_payload(PAYLOAD).expect("payload terbaca");

        assert_eq!(fetched.comments.len(), 3);
        let first = &fetched.comments[0];
        assert_eq!(first.name.as_str(), "Achmad Mario");
        assert_eq!(first.status.as_str(), "Hadir");
        assert_eq!(first.color.as_str(), "#048658");
        assert_eq!(first.date.to_rfc3339(), "2026-08-24T02:40:00+00:00");
    }

    #[test]
    fn spasi_tepi_dipangkas_dan_angka_jadi_teks() {
        let fetched = parse_payload(PAYLOAD).expect("payload terbaca");
        let names: Vec<&str> = fetched
            .comments
            .iter()
            .map(|comment| comment.name.as_str())
            .collect();

        assert!(names.contains(&"Nama Berspasi"), "dapat: {names:?}");
        assert!(names.contains(&"12345"), "dapat: {names:?}");
    }

    #[test]
    fn baris_cacat_dilewati_dengan_alasan_bukan_menggagalkan_semuanya() {
        let fetched = parse_payload(PAYLOAD).expect("payload terbaca");

        assert_eq!(fetched.skipped.len(), 2);
        assert!(fetched.skipped[0].reason.contains("name"));
        assert_eq!(fetched.skipped[1].name, "Tanggal Rusak");
        assert!(fetched.skipped[1].reason.contains("date"));
    }

    #[test]
    fn json_ngawur_ditolak_sebagai_respons_tidak_sesuai() {
        let err = parse_payload("<html>Service unavailable</html>").expect_err("bukan JSON");
        assert!(
            matches!(err, SourceError::InvalidResponse(_)),
            "dapat: {err:?}"
        );
    }

    #[test]
    fn payload_tanpa_field_comentar_menghasilkan_daftar_kosong() {
        let fetched = parse_payload(r##"{"status":200,"message":"kosong"}"##).expect("terbaca");
        assert!(fetched.comments.is_empty() && fetched.skipped.is_empty());
    }
}
