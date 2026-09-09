//! Entity `Comment` beserta value object-nya.
//!
//! Prinsipnya "parse, don't validate": setiap nilai dibungkus tipe sendiri yang
//! hanya bisa dibuat lewat konstruktor. Begitu sebuah `Comment` terbentuk,
//! seluruh isinya dijamin valid — layer lain tidak perlu mengecek ulang.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::error::DomainError;

const MAX_NAME_LEN: usize = 100;
const MAX_STATUS_LEN: usize = 32;
// Dinaikkan dari 1.000: endpoint lama tidak punya batas, dan dengan
// `mode: 'no-cors'` frontend tidak bisa membaca error — pesan yang ditolak
// akan hilang diam-diam tanpa tamu menyadarinya.
const MAX_MESSAGE_LEN: usize = 2_000;
const MAX_COLOR_LEN: usize = 32;
const DEFAULT_COLOR: &str = "#000000";

/// Nama pengirim komentar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentName(String);

/// Status kehadiran/kategori komentar (mis. `hadir`, `tidak hadir`).
///
/// Sengaja tetap berupa teks bebas, bukan enum, supaya nilai lama dari
/// spreadsheet tetap bisa dipindahkan apa adanya. Kalau daftar nilainya sudah
/// pasti, tipe ini tinggal diganti enum tanpa menyentuh layer lain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentStatus(String);

/// Isi pesan komentar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentMessage(String);

/// Warna tampilan komentar: hex (`#fff`, `#a1b2c3`) atau nama warna CSS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentColor(String);

impl CommentName {
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, DomainError> {
        Ok(Self(parse_required_text(
            "name",
            raw.as_ref(),
            MAX_NAME_LEN,
        )?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl CommentStatus {
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, DomainError> {
        Ok(Self(parse_required_text(
            "status",
            raw.as_ref(),
            MAX_STATUS_LEN,
        )?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl CommentMessage {
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, DomainError> {
        Ok(Self(parse_required_text(
            "message",
            raw.as_ref(),
            MAX_MESSAGE_LEN,
        )?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl CommentColor {
    pub fn parse(raw: impl AsRef<str>) -> Result<Self, DomainError> {
        let value = parse_required_text("color", raw.as_ref(), MAX_COLOR_LEN)?;
        if is_hex_color(&value) || is_color_keyword(&value) {
            Ok(Self(value))
        } else {
            Err(DomainError::validation(
                "color",
                "harus berupa hex (#fff / #a1b2c3) atau nama warna CSS",
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for CommentColor {
    fn default() -> Self {
        Self(DEFAULT_COLOR.to_owned())
    }
}

/// Komentar yang sudah tersimpan — identitas dan jejak waktunya lengkap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    pub id: Uuid,
    pub name: CommentName,
    pub status: CommentStatus,
    pub message: CommentMessage,
    pub color: CommentColor,
    /// Waktu komentar menurut pengirim (kolom `date` di versi spreadsheet).
    pub date: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Comment {
    /// Merakit ulang entity dari data mentah (dipakai repository saat membaca
    /// baris database). Data yang tidak lolos validasi ditolak di sini juga,
    /// jadi baris rusak tidak pernah bocor ke layer atas.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: Uuid,
        name: &str,
        status: &str,
        message: &str,
        color: &str,
        date: DateTime<Utc>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Result<Self, DomainError> {
        Ok(Self {
            id,
            name: CommentName::parse(name)?,
            status: CommentStatus::parse(status)?,
            message: CommentMessage::parse(message)?,
            color: CommentColor::parse(color)?,
            date,
            created_at,
            updated_at,
        })
    }
}

/// Data untuk membuat komentar baru — belum punya id maupun jejak waktu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewComment {
    pub name: CommentName,
    pub status: CommentStatus,
    pub message: CommentMessage,
    pub color: CommentColor,
    pub date: DateTime<Utc>,
}

impl NewComment {
    pub fn new(
        name: &str,
        status: &str,
        message: &str,
        color: Option<&str>,
        date: Option<DateTime<Utc>>,
    ) -> Result<Self, DomainError> {
        Ok(Self {
            name: CommentName::parse(name)?,
            status: CommentStatus::parse(status)?,
            message: CommentMessage::parse(message)?,
            color: color
                .map(CommentColor::parse)
                .transpose()?
                .unwrap_or_default(),
            date: date.unwrap_or_else(Utc::now),
        })
    }
}

/// Perubahan sebagian (`PUT` bergaya patch): field `None` berarti biarkan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpdateComment {
    pub name: Option<CommentName>,
    pub status: Option<CommentStatus>,
    pub message: Option<CommentMessage>,
    pub color: Option<CommentColor>,
    pub date: Option<DateTime<Utc>>,
}

impl UpdateComment {
    pub fn new(
        name: Option<&str>,
        status: Option<&str>,
        message: Option<&str>,
        color: Option<&str>,
        date: Option<DateTime<Utc>>,
    ) -> Result<Self, DomainError> {
        Ok(Self {
            name: name.map(CommentName::parse).transpose()?,
            status: status.map(CommentStatus::parse).transpose()?,
            message: message.map(CommentMessage::parse).transpose()?,
            color: color.map(CommentColor::parse).transpose()?,
            date,
        })
    }

    /// `true` kalau tidak ada satu pun field yang diubah.
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.status.is_none()
            && self.message.is_none()
            && self.color.is_none()
            && self.date.is_none()
    }
}

fn parse_required_text(
    field: &'static str,
    raw: &str,
    max_len: usize,
) -> Result<String, DomainError> {
    let value = raw.trim();
    if value.is_empty() {
        return Err(DomainError::validation(field, "wajib diisi"));
    }
    if value.chars().count() > max_len {
        return Err(DomainError::validation(
            field,
            format!("maksimal {max_len} karakter"),
        ));
    }
    Ok(value.to_owned())
}

fn is_hex_color(value: &str) -> bool {
    let Some(digits) = value.strip_prefix('#') else {
        return false;
    };
    matches!(digits.len(), 3 | 6) && digits.chars().all(|c| c.is_ascii_hexdigit())
}

fn is_color_keyword(value: &str) -> bool {
    value.starts_with(|c: char| c.is_ascii_alphabetic())
        && value.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || c == '-'
                || c == '('
                || c == ')'
                || c == ','
                || c == ' '
                || c == '%'
                || c == '.'
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_dipangkas_spasinya() {
        let name = CommentName::parse("  Arta  ").expect("nama valid");
        assert_eq!(name.as_str(), "Arta");
    }

    #[test]
    fn name_kosong_ditolak() {
        let err = CommentName::parse("   ").unwrap_err();
        assert_eq!(err, DomainError::validation("name", "wajib diisi"));
    }

    #[test]
    fn message_terlalu_panjang_ditolak() {
        let err = CommentMessage::parse("a".repeat(MAX_MESSAGE_LEN + 1)).unwrap_err();
        assert!(err.to_string().contains("maksimal"));
    }

    #[test]
    fn color_menerima_hex_dan_nama_warna() {
        assert!(CommentColor::parse("#fff").is_ok());
        assert!(CommentColor::parse("#A1B2C3").is_ok());
        assert!(CommentColor::parse("tomato").is_ok());
        assert!(CommentColor::parse("#12345").is_err());
        assert!(CommentColor::parse("###").is_err());
    }

    #[test]
    fn color_default_hitam() {
        let comment = NewComment::new("Arta", "hadir", "halo", None, None).expect("valid");
        assert_eq!(comment.color.as_str(), DEFAULT_COLOR);
    }

    #[test]
    fn update_kosong_terdeteksi() {
        let empty = UpdateComment::default();
        assert!(empty.is_empty());

        let filled = UpdateComment::new(None, None, Some("halo"), None, None).expect("valid");
        assert!(!filled.is_empty());
    }
}
