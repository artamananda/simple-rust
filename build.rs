//! Build script: menyuntikkan metadata build (commit git & waktu build) ke binary
//! sehingga `GET /version` bisa memverifikasi versi mana yang sedang berjalan.
//!
//! Skrip deploy mengekspor GIT_COMMIT/BUILD_TIME; kalau tidak ada, nilainya
//! dihitung di sini (berguna saat `cargo run` di lokal).

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=migrations");
    println!("cargo:rerun-if-env-changed=GIT_COMMIT");
    println!("cargo:rerun-if-env-changed=BUILD_TIME");

    let commit = from_env("GIT_COMMIT").unwrap_or_else(git_commit);
    let build_time = from_env("BUILD_TIME")
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string());

    println!("cargo:rustc-env=GIT_COMMIT={commit}");
    println!("cargo:rustc-env=BUILD_TIME={build_time}");
}

fn from_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn git_commit() -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}
