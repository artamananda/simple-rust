#!/usr/bin/env bash
# Deploy dev: menaikkan versi, build, kirim ke home server (tanpa nginx —
# hostname diarahkan lewat Cloudflare Zero Trust Tunnel).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
source "$SCRIPT_DIR/_common.sh"
source "$ROOT_DIR/keys/deploy-dev.conf"

APP_NAME="simple-rust-dev"
REMOTE_DIR="/var/www/html/apps/$APP_NAME"
DEPLOY_ARCH="${DEPLOY_ARCH:-arm64}"
TARGET="$(rust_target "$DEPLOY_ARCH")"

cd "$ROOT_DIR"

echo "==> [1/6] Naikkan versi & build binary ($DEPLOY_ARCH)..."
# Commit versi ditunda sampai deploy sukses (langkah terakhir) supaya deploy
# yang gagal tidak meninggalkan commit rilis.
VERSION="$(bump_version)"
build_binary "$TARGET" "$VERSION" "$APP_NAME"

echo "==> [2/6] Copy .env dev..."
cp "$ROOT_DIR/keys/.env.dev" .env

echo "==> [3/6] Pastikan direktori remote ada..."
ssh -i "$SSH_KEY" -p "$VPS_PORT" "$VPS_USER@$VPS_HOST" \
    "sudo mkdir -p $REMOTE_DIR && sudo chown -R $VPS_USER:$VPS_USER $REMOTE_DIR"

echo "==> [4/6] Upload binary, .env, dan service..."
scp -i "$SSH_KEY" -P "$VPS_PORT" \
    "$APP_NAME" \
    "$VPS_USER@$VPS_HOST:$REMOTE_DIR/$APP_NAME.new"
scp -i "$SSH_KEY" -P "$VPS_PORT" \
    .env \
    deploy/simple-rust-dev.service \
    "$VPS_USER@$VPS_HOST:$REMOTE_DIR/"

echo "==> [5/6] Bersihkan artefak lokal..."
rm -f "$APP_NAME" .env

echo "==> [6/6] Pasang service di home server..."
ssh -i "$SSH_KEY" -p "$VPS_PORT" "$VPS_USER@$VPS_HOST" bash <<EOSSH
set -e
cd $REMOTE_DIR

echo "  -> stop service & tukar binary"
sudo systemctl stop $APP_NAME.service || true
mv -f $APP_NAME.new $APP_NAME
chmod +x $APP_NAME

echo "  -> pasang & restart systemd service"
sudo cp -f simple-rust-dev.service /etc/systemd/system/$APP_NAME.service
sudo systemctl daemon-reload
sudo systemctl enable --now $APP_NAME.service
sudo systemctl restart $APP_NAME.service

echo "  -> status service"
sudo systemctl status $APP_NAME.service --no-pager -l
EOSSH

# Deploy sukses — baru catat versi ke git.
git -C "$ROOT_DIR" commit -m "chore(dev): release v$VERSION" -- Cargo.toml Cargo.lock || \
  echo "  (lewati commit — tidak ada perubahan atau git tidak tersedia)"

echo ""
echo "Backend (dev) deployed successfully (v$VERSION)"
echo "   Service listen di 127.0.0.1:8801 — arahkan hostname ke port itu di Cloudflare Zero Trust Tunnel."
