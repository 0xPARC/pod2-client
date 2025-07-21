# Build all JavaScript/TypeScript components
js-build:
    pnpm build

# Format all code (JavaScript and Rust)
format:
    pnpm format
    cargo fmt

# Run client in development mode
client-dev:
    cd apps/client && pnpm tauri dev

client-build:
    cd apps/client && pnpm tauri build

# PodNet commands
podnet-server:
    cargo run --profile release-with-debug -p podnet-server

podnet-cli *args:
    cargo run --profile release-with-debug -p podnet-cli -- {{ args }}

podnet-identity:
    cargo run --profile release-with-debug -p podnet-ident-strawman

# Build entire workspace
build-all:
    cargo build

# Run full development environment (all services + desktop app)
dev-all:
    pnpm exec mprocs

# Run just the core services (no desktop app)
dev-services:
    pnpm exec mprocs --names podnet-server,podnet-identity
