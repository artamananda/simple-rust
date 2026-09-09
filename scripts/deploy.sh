#!/usr/bin/env bash
# Deploy production: build binary Linux, kirim ke VPS, pasang systemd + nginx.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
source "$SCRIPT_DIR/_common.sh"
source "$ROOT_DIR/keys/deploy.conf"

APP_NAME="simple-rust"
REMOTE_DIR="/var/www/html/apps/$APP_NAME"
DEPLOY_ARCH="${DEPLOY_ARCH:-amd64}"
TARGET="$(rust_target "$DEPLOY_ARCH")"

cd "$ROOT_DIR"

# Versi tidak dinaikkan di sini — penomoran dilakukan di deploy dev.
# Production hanya merilis versi yang sudah diuji & di-commit.
VERSION="$(read_version)"

echo "==> [1/5] Cek arsitektur & direktori di VPS..."
# Dicek lebih dulu: binary dengan arsitektur salah baru ketahuan saat systemd
# menjalankannya (status=203/EXEC), padahal service lama sudah terlanjur mati.
REMOTE_ARCH="$(ssh -i "$SSH_KEY" -p "$VPS_PORT" "$VPS_USER@$VPS_HOST" "uname -m")"
case "$REMOTE_ARCH" in
    x86_64)  SERVER_ARCH="amd64" ;;
    aarch64) SERVER_ARCH="arm64" ;;
    *)       SERVER_ARCH="$REMOTE_ARCH" ;;
esac

if [[ "$SERVER_ARCH" != "$DEPLOY_ARCH" ]]; then
    echo "VPS berarsitektur $REMOTE_ARCH ($SERVER_ARCH), tapi build diminta untuk $DEPLOY_ARCH." >&2
    echo "Jalankan ulang: DEPLOY_ARCH=$SERVER_ARCH $0" >&2
    exit 1
fi
echo "  -> $REMOTE_ARCH, cocok dengan build $DEPLOY_ARCH"

# Direktori tujuan disiapkan di sini supaya deploy pertama tidak gagal di scp.
ssh -i "$SSH_KEY" -p "$VPS_PORT" "$VPS_USER@$VPS_HOST" \
    "sudo mkdir -p $REMOTE_DIR && sudo chown -R $VPS_USER:$VPS_USER $REMOTE_DIR && sudo chmod 755 $REMOTE_DIR"

echo "==> [2/5] Build binary release ($DEPLOY_ARCH)..."
build_binary "$TARGET" "$VERSION" "$APP_NAME"

echo "==> [3/5] Upload binary, .env, config, dan service ke VPS..."
# Binary dikirim ke nama sementara: file biner yang sedang berjalan tidak bisa
# ditimpa langsung (Linux ETXTBSY / "Text file busy"). Tukar nama di VPS.
scp -i "$SSH_KEY" -P "$VPS_PORT" \
    "$APP_NAME" \
    "$VPS_USER@$VPS_HOST:$REMOTE_DIR/$APP_NAME.new"
scp -i "$SSH_KEY" -P "$VPS_PORT" \
    deploy/simple-rust-production.conf \
    deploy/simple-rust.service \
    "$VPS_USER@$VPS_HOST:$REMOTE_DIR/"
# .env dikirim langsung dari keys/ dan diganti namanya di tujuan.
# Sebelumnya file ini disalin dulu ke root repo lalu dihapus saat cleanup —
# yang berarti setiap deploy ikut menghapus .env development milik kita.
scp -i "$SSH_KEY" -P "$VPS_PORT" \
    "$ROOT_DIR/keys/.env.production" \
    "$VPS_USER@$VPS_HOST:$REMOTE_DIR/.env"

echo "==> [4/5] Bersihkan binary lokal..."
rm -f "$APP_NAME"

echo "==> [5/5] Pasang service & reload nginx di VPS..."
ssh -i "$SSH_KEY" -p "$VPS_PORT" "$VPS_USER@$VPS_HOST" bash <<EOSSH
set -e
cd $REMOTE_DIR

echo "  -> stop service & tukar binary"
sudo systemctl stop $APP_NAME.service || true

# Kalau $APP_NAME kebetulan sebuah direktori, "mv -f" tidak menimpanya —
# ia memindahkan biner KE DALAM direktori itu, dan ExecStart lalu menunjuk
# direktori (systemd melaporkannya sebagai status=203/EXEC).
if [ -d "$APP_NAME" ]; then
    echo "ERROR: $REMOTE_DIR/$APP_NAME adalah direktori, bukan file biner." >&2
    echo "Pindahkan/hapus direktori itu lebih dulu, lalu deploy ulang." >&2
    exit 1
fi

mv -f $APP_NAME.new $APP_NAME
# chmod 755, bukan "chmod +x": kalau umask di server bikin file jadi 600,
# "+x" hanya menghasilkan 700 dan service (jalan sebagai www-data) kena
# Permission denied yang muncul sebagai status=203/EXEC.
chmod 755 $APP_NAME
# .env berisi kredensial. Cukup 600 milik user deploy: systemd (PID 1, root)
# yang membacanya lalu meneruskan isinya ke proses, jadi www-data tidak perlu
# akses — dan file tetap bisa ditimpa scp pada deploy berikutnya.
chmod 600 .env

echo "  -> pastikan binary bisa dijalankan service user"
sudo -u www-data test -f $REMOTE_DIR/$APP_NAME -a -x $REMOTE_DIR/$APP_NAME || {
    echo "www-data tidak bisa mengeksekusi $REMOTE_DIR/$APP_NAME — cek izin folder induknya:" >&2
    namei -l $REMOTE_DIR/$APP_NAME >&2 || true
    exit 1
}

echo "  -> pasang nginx config"
sudo cp -f simple-rust-production.conf /etc/nginx/conf.d/simple-rust-production.conf
sudo nginx -t
sudo systemctl reload nginx

echo "  -> pasang & restart systemd service"
sudo cp -f simple-rust.service /etc/systemd/system/$APP_NAME.service
sudo systemctl daemon-reload
sudo systemctl enable --now $APP_NAME.service
sudo systemctl restart $APP_NAME.service

echo "  -> status service"
sudo systemctl status $APP_NAME.service --no-pager -l
EOSSH

echo ""
echo "Backend deployed successfully (v$VERSION) -> https://api-annisaarta.novelle.id"
echo "   Cek hasil deploy: curl https://api-annisaarta.novelle.id/version"
