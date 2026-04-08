set shell := ["zsh", "-lc"]

default:
    @just --list

server:
    cargo run -p crm-server

test:
    cargo test --workspace

crm-app-test:
    cargo test -p crm-app

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

desktop-build:
    cd apps/desktop && bun run build

desktop-rust-check:
    cd apps/desktop/src-tauri && cargo check --offline

desktop-rust-fmt:
    cd apps/desktop/src-tauri && cargo fmt

desktop:
    just desktop-dev

extension-install:
    cd apps/extension && bun install

extension-build:
    cd apps/extension && bun run build

extension-dev:
    cd apps/extension && bun run dev

info:
    @echo "bun:   $(bun --version)"
    @echo "cargo: $(cargo --version)"
    @echo "rustc: $(rustc --version)"
