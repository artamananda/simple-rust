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

echo "==> [1/5] Naikkan versi & build binary ($DEPLOY_ARCH)..."
# Commit versi ditunda sampai deploy sukses (langkah terakhir) supaya deploy
# yang gagal tidak meninggalkan commit rilis.
VERSION="$(bump_version)"
build_binary "$TARGET" "$VERSION" "$APP_NAME"

echo "==> [2/5] Pastikan direktori remote ada..."
ssh -i "$SSH_KEY" -p "$VPS_PORT" "$VPS_USER@$VPS_HOST" \
    "sudo mkdir -p $REMOTE_DIR && sudo chown -R $VPS_USER:$VPS_USER $REMOTE_DIR"

echo "==> [3/5] Upload binary, .env, dan service..."
scp -i "$SSH_KEY" -P "$VPS_PORT" \
    "$APP_NAME" \
    "$VPS_USER@$VPS_HOST:$REMOTE_DIR/$APP_NAME.new"
scp -i "$SSH_KEY" -P "$VPS_PORT" \
    deploy/simple-rust-dev.service \
    "$VPS_USER@$VPS_HOST:$REMOTE_DIR/"
# .env dikirim langsung dari keys/ dan diganti namanya di tujuan.
# Sebelumnya file ini disalin dulu ke root repo lalu dihapus saat cleanup —
# yang berarti setiap deploy ikut menghapus .env development milik kita.
scp -i "$SSH_KEY" -P "$VPS_PORT" \
    "$ROOT_DIR/keys/.env.dev" \
    "$VPS_USER@$VPS_HOST:$REMOTE_DIR/.env"

echo "==> [4/5] Bersihkan binary lokal..."
rm -f "$APP_NAME"

echo "==> [5/5] Pasang service di home server..."
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
# chmod 755, bukan "chmod +x" — lihat catatan di deploy.sh.
chmod 755 $APP_NAME
# .env berisi kredensial. Cukup 600 milik user deploy: systemd (PID 1, root)
# yang membacanya lalu meneruskan isinya ke proses, jadi www-data tidak perlu
# akses — dan file tetap bisa ditimpa scp pada deploy berikutnya.
chmod 600 .env

echo "  -> pastikan binary bisa dijalankan service user"
sudo -u www-data test -f $REMOTE_DIR/$APP_NAME -a -x $REMOTE_DIR/$APP_NAME || {
    echo "www-data tidak bisa mengeksekusi $REMOTE_DIR/$APP_NAME:" >&2
    namei -l $REMOTE_DIR/$APP_NAME >&2 || true
    exit 1
}

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
