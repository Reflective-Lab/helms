set shell := ["zsh", "-lc"]

default:
    @just --list

server:
    cargo run -p application-server

test:
    cargo test --workspace

workbench-backend-test:
    cargo test -p workbench-backend

crm-app-test:
    cargo test -p workbench-backend

fmt:
    cargo fmt --all

desktop-install:
    cd apps/desktop && bun install

desktop-check:
    cd apps/desktop && bun run check

desktop-web:
    cd apps/desktop && bun run dev:web

desktop-build-web:
    cd apps/desktop && bun run build:web

desktop-dev:
    cd apps/desktop && bun run dev

desktop-dev-remote:
    cd apps/desktop && bun run dev:remote

desktop-build:
    cd apps/desktop && bun run build

desktop-build-remote:
    cd apps/desktop && bun run build:remote

desktop-rust-check:
    cd apps/desktop/src-tauri && cargo check --offline

desktop-rust-check-remote:
    cd apps/desktop/src-tauri && cargo check --no-default-features --offline

desktop-rust-fmt:
    cd apps/desktop/src-tauri && cargo fmt

desktop:
    just desktop-dev

import-apple-notes:
    CARGO_TARGET_DIR=/tmp/prio-apple-notes-cli cargo run -p prio-apple-notes-cli

truth-resolution:
    cargo run -p prio-truths --example real-truth-resolution --

extension-install:
    cd apps/extension && bun install

extension-build:
    cd apps/extension && bun run build

extension-dev:
    cd apps/extension && bun run dev

gen-seed-data:
    cargo run -p seed-gen

info:
    @echo "bun:   $(bun --version)"
    @echo "cargo: $(cargo --version)"
    @echo "rustc: $(rustc --version)"
