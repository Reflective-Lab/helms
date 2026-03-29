set shell := ["zsh", "-lc"]

server:
    cargo run -p crm-server

test:
    cargo test --workspace

fmt:
    cargo fmt --all

desktop:
    cd apps/desktop && npm run dev

