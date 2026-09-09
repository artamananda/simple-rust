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

echo "==> [1/6] Cek arsitektur & direktori di VPS..."
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

echo "==> [2/6] Build binary release ($DEPLOY_ARCH)..."
build_binary "$TARGET" "$VERSION" "$APP_NAME"

echo "==> [3/6] Copy .env production..."
cp "$ROOT_DIR/keys/.env.production" .env

echo "==> [4/6] Upload binary, .env, config, dan service ke VPS..."
# Binary dikirim ke nama sementara: file biner yang sedang berjalan tidak bisa
# ditimpa langsung (Linux ETXTBSY / "Text file busy"). Tukar nama di VPS.
scp -i "$SSH_KEY" -P "$VPS_PORT" \
    "$APP_NAME" \
    "$VPS_USER@$VPS_HOST:$REMOTE_DIR/$APP_NAME.new"
scp -i "$SSH_KEY" -P "$VPS_PORT" \
    .env \
    deploy/simple-rust-production.conf \
    deploy/simple-rust.service \
    "$VPS_USER@$VPS_HOST:$REMOTE_DIR/"

echo "==> [5/6] Bersihkan artefak lokal..."
rm -f "$APP_NAME" .env

echo "==> [6/6] Pasang service & reload nginx di VPS..."
ssh -i "$SSH_KEY" -p "$VPS_PORT" "$VPS_USER@$VPS_HOST" bash <<EOSSH
set -e
cd $REMOTE_DIR

echo "  -> stop service & tukar binary"
sudo systemctl stop $APP_NAME.service || true
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
sudo -u www-data test -x $REMOTE_DIR/$APP_NAME || {
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
echo "Backend deployed successfully (v$VERSION) -> https://api.annisaarta.novelle.id"
echo "   Cek hasil deploy: curl https://api.annisaarta.novelle.id/version"
