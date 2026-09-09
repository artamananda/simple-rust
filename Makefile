.PHONY: help run test fmt lint check db-create db-drop

help: ## Tampilkan daftar perintah
	@grep -E "^[a-z-]+:.*##" $(MAKEFILE_LIST) | sed "s/:.*## /\t/"

run: ## Jalankan API (migrasi ikut jalan otomatis)
	cargo run

test: ## Jalankan seluruh test (tidak butuh database)
	cargo test

fmt: ## Rapikan format kode
	cargo fmt --all

lint: ## Clippy, warning dianggap error
	cargo clippy --all-targets --all-features -- -D warnings

check: fmt lint test ## Format + lint + test sebelum commit

db-create: ## Buat database lokal
	createdb simple_rust_local

db-drop: ## Hapus database lokal
	dropdb --if-exists simple_rust_local
