set shell := ["zsh", "-lc"]

default:
    @just --list

# ── CI ─────────────────────────────────────────────────────────────────

# Canonical CI aggregate (RP-CI-PARITY): CI runs exactly `just ci`.
ci: fmt-check check lint test

# ── Build ──────────────────────────────────────────────────────────────

build-desktop-web:
    cd apps/desktop && bun run build:web

build-desktop:
    cd apps/desktop && bun run build

build-desktop-remote:
    cd apps/desktop && bun run build:remote

build-extension:
    cd apps/extension && bun run build

# ── Test ───────────────────────────────────────────────────────────────

test:
    cargo test --workspace --all-targets

test-workbench-backend:
    cargo test -p workbench-backend

test-crm-app:
    @echo "Compatibility alias for test-workbench-backend."
    cargo test -p workbench-backend

# ── Lint & Format ──────────────────────────────────────────────────────

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

check:
    cargo check --workspace --all-targets

lint:
    cargo clippy --workspace --all-targets -- -D warnings

desktop-rust-fmt:
    cd apps/desktop/src-tauri && cargo fmt

desktop-check:
    cd apps/desktop && bun run check

desktop-rust-check:
    cd apps/desktop/src-tauri && cargo check --offline

desktop-rust-check-remote:
    cd apps/desktop/src-tauri && cargo check --no-default-features --offline

# ── Dev ────────────────────────────────────────────────────────────────

dev-server:
    @echo "No standalone application-layer server crate exists now; use dev-desktop or test-workbench-backend."

dev-desktop-install:
    cd apps/desktop && bun install

dev-desktop:
    cd apps/desktop && bun run dev

dev-desktop-web:
    cd apps/desktop && bun run dev:web

dev-desktop-remote:
    cd apps/desktop && bun run dev:remote

dev-extension-install:
    cd apps/extension && bun install

dev-extension:
    cd apps/extension && bun run dev

dev-import-apple-notes:
    CARGO_TARGET_DIR=/tmp/prio-apple-notes-cli cargo run -p prio-apple-notes-cli

dev-truth-resolution:
    cargo run -p truth-catalog --example real-truth-resolution --

dev-seed-data:
    cargo run -p seed-gen

# ── Info ───────────────────────────────────────────────────────────────

info:
    @echo "bun:   $(bun --version)"
    @echo "cargo: $(cargo --version)"
    @echo "rustc: $(rustc --version)"
