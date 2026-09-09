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

echo "==> [1/5] Build binary release ($DEPLOY_ARCH)..."
build_binary "$TARGET" "$VERSION" "$APP_NAME"

echo "==> [2/5] Copy .env production..."
cp "$ROOT_DIR/keys/.env.production" .env

echo "==> [3/5] Upload binary, .env, config, dan service ke VPS..."
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

echo "==> [4/5] Bersihkan artefak lokal..."
rm -f "$APP_NAME" .env

echo "==> [5/5] Pasang service & reload nginx di VPS..."
ssh -i "$SSH_KEY" -p "$VPS_PORT" "$VPS_USER@$VPS_HOST" bash <<EOSSH
set -e
cd $REMOTE_DIR

echo "  -> stop service & tukar binary"
sudo systemctl stop $APP_NAME.service || true
mv -f $APP_NAME.new $APP_NAME
chmod +x $APP_NAME

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
echo "Backend deployed successfully (v$VERSION) -> https://api.simple-rust.artamananda.my.id"
echo "   Cek hasil deploy: curl https://api.simple-rust.artamananda.my.id/version"
