-- Tabel `comments` — pengganti sheet "comment" di Google Spreadsheet.
--
-- Kolom `date` di sheet dipetakan ke `commented_at` (kata `date` juga nama tipe
-- di Postgres, jadi dihindari sebagai nama kolom). API tetap mengekspornya
-- sebagai field `date` lewat layer DTO.

CREATE EXTENSION IF NOT EXISTS "pgcrypto";

CREATE TABLE IF NOT EXISTS comments (
    id           UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    name         TEXT        NOT NULL,
    status       TEXT        NOT NULL,
    message      TEXT        NOT NULL,
    color        TEXT        NOT NULL DEFAULT '#000000',
    commented_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Urutan default endpoint list: komentar terbaru lebih dulu.
CREATE INDEX IF NOT EXISTS idx_comments_commented_at ON comments (commented_at DESC);

-- Filter ?status=...
CREATE INDEX IF NOT EXISTS idx_comments_status ON comments (status);
