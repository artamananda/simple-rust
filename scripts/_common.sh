#!/usr/bin/env bash
# Fungsi yang dipakai bersama oleh deploy.sh dan deploy-dev.sh.

# Menerjemahkan arsitektur VPS (amd64/arm64) ke target Rust.
# Dipakai target musl agar binary statis: tidak ada ketergantungan versi glibc
# di server, jadi binary yang sama jalan di Debian/Ubuntu versi apa pun.
rust_target() {
    local arch="$1"
    case "$arch" in
        amd64|x86_64)  echo "x86_64-unknown-linux-musl" ;;
        arm64|aarch64) echo "aarch64-unknown-linux-musl" ;;
        *)
            echo "Arsitektur '$arch' tidak dikenal (pakai amd64 atau arm64)." >&2
            return 1
            ;;
    esac
}

# Build release untuk target Linux.
#
# macOS tidak bisa langsung meng-compile untuk Linux (butuh linker silang),
# jadi dipakai cargo-zigbuild — atau `cross` kalau itu yang tersedia.
#   Pasang sekali:  brew install zig && cargo install cargo-zigbuild
#                   rustup target add x86_64-unknown-linux-musl
build_binary() {
    local target="$1" version="$2" out_name="$3"

    rustup target list --installed | grep -qx "$target" \
        || rustup target add "$target"

    export GIT_COMMIT="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
    export BUILD_TIME="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "  -> v$version ($GIT_COMMIT) @ $BUILD_TIME [$target]"

    if [[ "$(uname -s)" == "Linux" && "$target" == "$(rustc -vV | awk '/host:/{print $2}')" ]]; then
        cargo build --release --target "$target"
    elif command -v cargo-zigbuild >/dev/null 2>&1; then
        cargo zigbuild --release --target "$target"
    elif command -v cross >/dev/null 2>&1; then
        cross build --release --target "$target"
    else
        cat >&2 <<'MSG'
Tidak ada toolchain untuk cross-compile ke Linux.

Pilih salah satu, cukup sekali pasang:
  brew install zig && cargo install cargo-zigbuild     # ringan, direkomendasikan
  cargo install cross                                  # butuh Docker berjalan
MSG
        return 1
    fi

    cp "target/$target/release/simple-rust" "$out_name"
}

# Versi dibaca dari [package].version di Cargo.toml — sumber kebenaran tunggal.
read_version() {
    awk '/^\[package\]/{p=1; next} /^\[/{p=0} p && /^version[[:space:]]*=/{gsub(/[",]/,"",$3); print $3; exit}' Cargo.toml
}

# Naikkan patch version (0.1.0 -> 0.1.1) di Cargo.toml dan Cargo.lock.
bump_version() {
    local current major minor patch next
    current="$(read_version)"
    IFS=. read -r major minor patch <<<"$current"
    next="${major}.${minor}.$((patch + 1))"

    # Hanya baris `version` pertama (milik [package]) yang diganti.
    # Catatan: `next` adalah keyword awk, jadi variabelnya dinamai `newver`.
    awk -v newver="$next" '
        /^\[package\]/ { in_pkg = 1 }
        /^\[/ && !/^\[package\]/ { in_pkg = 0 }
        in_pkg && /^version[[:space:]]*=/ && !done { print "version = \"" newver "\""; done = 1; next }
        { print }
    ' Cargo.toml > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml

    cargo update --offline --package simple-rust >/dev/null 2>&1 || true
    echo "$next"
}
