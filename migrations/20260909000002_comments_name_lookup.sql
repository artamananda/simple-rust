-- Sinkronisasi memakai nama sebagai kunci, dengan pencocokan yang mengabaikan
-- huruf besar/kecil dan spasi di ujung. Index fungsional ini membuat pencarian
-- tersebut tetap murah saat jumlah komentar bertambah.
CREATE INDEX IF NOT EXISTS idx_comments_name_lookup
    ON comments (lower(btrim(name)));
