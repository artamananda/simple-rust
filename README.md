# simple-rust

API komentar (buku tamu) di atas **Rust + Axum + PostgreSQL**, disusun dengan
*clean architecture*. Menggantikan endpoint Apps Script + Spreadsheet yang lambat:
respons `GET /api/comments` di mesin lokal ~**1 ms**, dibanding ratusan hingga
ribuan milidetik lewat Apps Script.

## Arsitektur

Arah dependensi selalu ke dalam — layer luar boleh tahu layer dalam, tidak sebaliknya:

```
presentation (HTTP/Axum) ─┐
                          ├─> application (use case) ─> domain (aturan bisnis)
infrastructure (Postgres)─┘
```

| Layer | Isi | Tahu apa? |
| --- | --- | --- |
| `domain` | entity `Comment`, value object, trait `CommentRepository` | tidak tahu HTTP maupun SQL |
| `application` | `CommentService` (use case) | hanya tahu trait domain |
| `infrastructure` | pool sqlx, `PgCommentRepository`, migrasi | tahu Postgres |
| `presentation` | router, handler, DTO, format respons | tahu HTTP |

Manfaat konkretnya ada di `tests/comment_service.rs`: seluruh use case diuji
dengan repository in-memory — **tanpa** Postgres, tanpa HTTP server, selesai
dalam milidetik.

```
src/
├── main.rs                 # entrypoint: baca config, rakit dependensi, jalankan server
├── lib.rs                  # deklarasi modul + metadata build
├── config.rs               # satu-satunya pembaca environment variable
├── domain/
│   ├── comment.rs          # entity + value object (validasi ada di sini)
│   ├── pagination.rs       # Pagination & Page
│   ├── repository.rs       # trait CommentRepository (kontrak, bukan implementasi)
│   ├── sync.rs             # trait CommentSource + laporan sinkronisasi
│   └── error.rs            # DomainError
├── application/
│   ├── comment_service.rs  # use case: list, get, create, update, delete
│   ├── sync_service.rs     # use case: sinkronisasi dari sumber luar
│   └── error.rs            # ServiceError (validasi / not found / upstream / tak terduga)
├── infrastructure/
│   ├── db.rs               # pool koneksi + migrasi
│   ├── http/apps_script_source.rs   # adapter ke endpoint Apps Script lama
│   └── postgres/comment_repository.rs
└── presentation/
    ├── router.rs           # rute + CORS + trace + timeout
    ├── handlers/           # comment.rs, health.rs
    ├── dto.rs              # bentuk JSON request/response
    ├── response.rs         # envelope { status, message, data, meta }
    └── error.rs            # pemetaan error -> status HTTP
```

### Kaidah Rust yang dipakai

- **Parse, don't validate** — `CommentName`, `CommentStatus`, `CommentMessage`,
  `CommentColor` hanya bisa dibuat lewat konstruktor yang memvalidasi. Begitu
  sebuah `Comment` terbentuk, isinya dijamin valid; layer lain tidak perlu
  mengecek ulang.
- **Error sebagai tipe** — `thiserror` di tiap layer (`DomainError`,
  `RepositoryError`, `ServiceError`), `anyhow` hanya untuk kegagalan teknis dan
  proses startup. Tidak ada `unwrap()` di jalur request.
- **Trait sebagai batas layer** — `CommentRepository` di-inject sebagai
  `Arc<dyn CommentRepository>`, jadi implementasinya bisa ditukar.
- **Entity ≠ baris database ≠ JSON** — `CommentRow` (sqlx) dan `CommentResponse`
  (serde) terpisah dari entity, sehingga nama kolom dan nama field API bebas berbeda.
- **Anti-corruption layer** — bentuk respons Apps Script (`comentar`, `id` angka,
  tanggal string) hanya dikenal di `infrastructure/http/apps_script_source.rs`.
  Sisa aplikasi tidak tahu data itu pernah tinggal di spreadsheet.

## Skema database

Migrasi ada di `migrations/` dan **ikut ter-embed di dalam binary**, lalu
dijalankan otomatis saat startup — deploy cukup mengirim satu file biner.

| Kolom | Tipe | Catatan |
| --- | --- | --- |
| `id` | `UUID` | dibuat server (`gen_random_uuid()`), bukan lagi dari klien |
| `name` | `TEXT` | wajib, maks. 100 karakter |
| `status` | `TEXT` | wajib, maks. 32 karakter — tetap teks bebas agar data lama dari sheet bisa masuk apa adanya |
| `message` | `TEXT` | wajib, maks. 1.000 karakter |
| `color` | `TEXT` | hex (`#fff`, `#a1b2c3`) atau nama warna CSS; default `#000000` |
| `commented_at` | `TIMESTAMPTZ` | kolom `date` versi sheet; di API tetap bernama `date` |
| `created_at` / `updated_at` | `TIMESTAMPTZ` | jejak waktu baris |

`date` dihindari sebagai nama kolom karena juga nama tipe di Postgres —
penerjemahannya ditangani layer DTO.

## Endpoint

Semua respons memakai satu envelope yang sama, termasuk saat error:

```json
{ "status": 200, "message": "Berhasil mengambil data", "data": [ ... ], "meta": { ... } }
```

| Method | Path | Keterangan |
| --- | --- | --- |
| `GET` | `/healthz` | health check |
| `GET` | `/version` | versi, commit, waktu build — untuk verifikasi hasil deploy |
| `GET` | `/api/comments` | daftar komentar (lihat query di bawah) |
| `GET` | `/api/comments/{id}` | detail satu komentar |
| `POST` | `/api/comments` | tambah komentar → `201` |
| `PUT` | `/api/comments/{id}` | ubah sebagian field |
| `DELETE` | `/api/comments/{id}` | hapus komentar |
| `POST` | `/api/comments/sync` | tarik data dari Apps Script lama, simpan dengan nama sebagai kunci |

**Query `GET /api/comments`**

| Parameter | Default | Keterangan |
| --- | --- | --- |
| `page` | `1` | nomor halaman |
| `perPage` | `20` | maks. `100` (alias: `per_page`, `limit`) |
| `status` | – | filter persis, mis. `?status=hadir` |
| `q` | – | cari di nama atau pesan (alias: `search`) |
| `sort` | `newest` | `newest` atau `oldest` |

Contoh:

```bash
# tambah komentar
curl -X POST http://localhost:8080/api/comments \
  -H 'Content-Type: application/json' \
  -d '{"name":"Arta","status":"hadir","message":"Selamat ya!","color":"#ff6b6b"}'

# ambil 10 komentar terbaru berstatus hadir
curl 'http://localhost:8080/api/comments?status=hadir&perPage=10'

# ubah status saja — field lain tidak tersentuh
curl -X PUT http://localhost:8080/api/comments/<id> \
  -H 'Content-Type: application/json' -d '{"status":"tidak hadir"}'
```

Error selalu berbentuk sama dan menyebut field yang bermasalah:

```json
{ "status": 400, "message": "name: wajib diisi" }
```

## Sinkronisasi dari Apps Script

Endpoint ini menarik seluruh komentar dari endpoint Apps Script lama lalu
memasukkannya ke Postgres, **dengan nama sebagai kunci** — aman dijalankan
berkali-kali tanpa menggandakan data.

```bash
curl -X POST http://localhost:8080/api/comments/sync \
  -H 'Content-Type: application/json' \
  -d '{"key":"jerapah"}'
```

```json
{
  "status": 200,
  "message": "Sinkronisasi selesai",
  "data": { "fetched": 22, "created": 22, "updated": 0, "skipped": [] }
}
```

Jalankan lagi dengan data yang sama, hasilnya `created: 0, updated: 22` —
itulah tanda sinkronisasinya idempoten.

**Aturan mainnya**

- **Kunci penjaga** dikirim di body sebagai `key` (boleh juga `secret` atau
  `token`), dicocokkan dengan `SYNC_SECRET`. Salah kunci → `401`.
- **Endpoint mati kalau belum dikonfigurasi.** Tanpa `SYNC_SOURCE_URL` *dan*
  `SYNC_SECRET`, jawabannya `503` — tidak ada kunci default yang bisa ditebak.
- **Pencocokan nama** mengabaikan huruf besar/kecil dan spasi di ujung: `"Budi "`
  dari sheet dianggap orang yang sama dengan `"budi"`. Ejaan terbaru dari sumber
  yang menang. (Data aslimu memang punya satu nama berspasi di ujung.)
- **Satu transaksi + advisory lock.** Kalau ada baris yang gagal, tidak ada yang
  setengah tersimpan; dua sync yang berjalan bersamaan akan antre, bukan
  sama-sama menyisipkan nama yang sama.
- **Baris cacat dilewati, bukan didiamkan.** Baris tanpa nama atau bertanggal
  tak terbaca masuk ke daftar `skipped` lengkap dengan alasannya, sementara
  baris lain tetap tersimpan:
  ```json
  "skipped": [{ "name": "Tanggal Rusak", "reason": "date: 'bukan tanggal' bukan tanggal RFC 3339" }]
  ```
- **`id` dari sheet diabaikan** — Postgres yang membuat UUID-nya.
- **Sumber bermasalah → `502`**, dibedakan dari error kita sendiri (`500`).
- Apps Script bisa lambat (pernah terukur **45 detik** untuk satu GET), jadi
  timeout klien longgar: `SYNC_TIMEOUT_SECONDS`, default 120 detik.

Sinkronisasi hanya menambah dan memperbarui; komentar yang dihapus di
spreadsheet **tidak** ikut terhapus di Postgres.

## Menjalankan di lokal

```bash
cp .env.example .env        # lalu sesuaikan DATABASE_URL
make db-create              # createdb simple_rust_local
make run                    # migrasi jalan otomatis, server listen di :8080
```

Perintah lain: `make test` (tanpa database), `make lint`, `make check`
(format + lint + test sebelum commit).

## Beda dengan versi Apps Script

Yang perlu disesuaikan di frontend:

1. **Nama field daftar komentar**: `comentar` → `data`.
   ```js
   // sebelumnya
   const { comentar } = await res.json();
   // sekarang
   const { data } = await res.json();
   ```
2. **`id` tidak lagi dikirim klien** — server yang membuat UUID. Kalau payload
   lama tetap menyertakan `id`, field itu diabaikan (tidak error).
3. **`status` wajib diisi**; `color` opsional (default `#000000`).
4. **`date` opsional** — kalau tidak dikirim, dipakai waktu server (RFC 3339 / ISO 8601).
5. `POST` berhasil menjawab **201**, bukan 200. Validasi gagal menjawab **400**,
   data tidak ada **404** — tidak lagi selalu 200 seperti Apps Script.

Memindahkan data lama: ekspor sheet ke JSON, lalu POST baris per baris ke
`/api/comments` (kolom `date` boleh ikut dikirim agar urutan aslinya terjaga).

## Deployment

Mengikuti pola `novelle/backend`: satu binary statis + systemd + nginx reverse proxy.

**Persiapan sekali saja**

```bash
cp keys/deploy.conf.example       keys/deploy.conf       # kredensial SSH VPS
cp keys/deploy.conf.example       keys/deploy-dev.conf   # kredensial home server
cp keys/.env.production.example   keys/.env.production   # .env yang dikirim ke VPS

# toolchain cross-compile macOS -> Linux (pilih salah satu)
brew install zig && cargo install cargo-zigbuild        # ringan, direkomendasikan
cargo install cross                                     # alternatif, butuh Docker
```

Isi folder `keys/` tidak ikut ter-commit (lihat `.gitignore`).

**Deploy**

```bash
./scripts/deploy-dev.sh    # naikkan versi patch, build arm64, kirim ke home server
./scripts/deploy.sh        # build amd64 versi terkini, kirim ke VPS + reload nginx
```

Penomoran versi dipusatkan di deploy dev (`bump_version` pada `Cargo.toml`,
di-commit hanya setelah deploy sukses); deploy production merilis versi yang
sudah teruji itu. Timpa arsitektur bila perlu: `DEPLOY_ARCH=arm64 ./scripts/deploy.sh`.

Target build adalah **musl** sehingga binary-nya statis — tidak ada
ketergantungan versi glibc di server.

**Yang dilakukan skrip deploy**

1. Build release untuk target Linux, dengan `GIT_COMMIT`/`BUILD_TIME` disuntikkan
   ke binary (terlihat di `GET /version`).
2. Kirim binary sebagai `<nama>.new` — file biner yang sedang berjalan tidak bisa
   ditimpa langsung di Linux (`ETXTBSY`); penukaran nama dilakukan di server.
3. Kirim `.env`, unit systemd, dan (production) config nginx.
4. Stop service → tukar binary → `daemon-reload` → restart → tampilkan status.

Migrasi database tidak perlu langkah terpisah: dijalankan otomatis saat service
menyala. Server merespons SIGTERM dengan *graceful shutdown*, jadi
`systemctl restart` tidak memutus request yang sedang berjalan.

Verifikasi setelah deploy:

```bash
curl https://api.simple-rust.artamananda.my.id/version
```

Ganti `server_name`, port (`127.0.0.1:8081`), dan nama host di
`deploy/simple-rust-production.conf` sesuai domain yang dipakai.
