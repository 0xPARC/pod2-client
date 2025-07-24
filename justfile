# Build all JavaScript/TypeScript components
js-build:
    pnpm build

# Format all code (JavaScript and Rust)
format:
    pnpm format
    cargo fmt

# Client development commands
# Default: dev mode with release build and staging servers (recommended for most development)
client-dev:
    cd apps/client && pnpm tauri dev --release -- -- --set network.document_server=https://pod-server.ghost-spica.ts.net/server-staging --set network.identity_server=https://pod-server.ghost-spica.ts.net/identity-staging --set database.name=staging.db

# Dev mode with debug build (slower, better for debugging)
client-dev-debug:
    cd apps/client && pnpm tauri dev -- -- --set network.document_server=https://pod-server.ghost-spica.ts.net/server-staging --set network.identity_server=https://pod-server.ghost-spica.ts.net/identity-staging --set database.name=staging.db

# Dev mode with local servers (requires running servers-local)
client-dev-local:
    cd apps/client && pnpm tauri dev --release -- -- --set network.document_server=http://localhost:3000 --set network.identity_server=http://localhost:3000 --set database.name=local.db

# Dev mode with production servers (testing against prod)
client-dev-prod:
    cd apps/client && pnpm tauri dev --release -- -- --set network.document_server=https://pod-server.ghost-spica.ts.net/server --set network.identity_server=https://pod-server.ghost-spica.ts.net/identity-staging --set database.name=prod.db

client-build:
    cd apps/client && pnpm tauri build

# Server management commands
# Run both document and identity servers locally (for client-dev-local)
servers-local:
    just podnet-server & just podnet-identity

# Individual server commands
podnet-server:
    cargo run --profile release-with-debug -p podnet-server

podnet-identity:
    cargo run --profile release-with-debug -p podnet-ident-strawman

# PodNet CLI
podnet-cli *args:
    cargo run --profile release-with-debug -p podnet-cli -- {{ args }}

# Build entire workspace
build-all:
    cargo build

# Complete development environments
# Full local development (client + local servers)
dev-local:
    echo "Starting local development environment..."
    echo "This will run the client with local servers"
    just servers-local & sleep 3 && just client-dev-local

# Full development environment (all services + desktop app) - uses mprocs
dev-all:
    pnpm exec mprocs

# Run just the core services (no desktop app)
dev-services:
    pnpm exec mprocs --names podnet-server,podnet-identity

# CI/Testing commands
ci-rust:
    echo "🦀 Running Rust CI checks..."
    cargo fmt --check
    cargo clippy --all-targets --all-features -- -D warnings
    cargo test --release

ci-js:
    echo "🌐 Running JavaScript/TypeScript CI checks..."
    pnpm install --frozen-lockfile
    pnpm build
    pnpm lint
    pnpm format:check
    pnpm test

# Run all CI checks locally (same as GitHub workflows)
ci-all:
    echo "🚀 Running all CI checks (Rust + JS)..."
    just ci-rust
    just ci-js
    echo "✅ All CI checks passed!"

# Quick checks (faster subset for development)
check-quick:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    pnpm lint
