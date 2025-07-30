# Build all JavaScript/TypeScript components
js-build:
    pnpm build

# Format and lint all code (JavaScript/TypeScript and Rust)
format:
    echo "🎨 Formatting and linting all code..."
    echo "📄 Formatting JavaScript/TypeScript files..."
    pnpm format
    echo "🔍 Linting JavaScript/TypeScript files..."
    pnpm lint
    echo "🚀 Formatting Rust code..."
    cargo fmt --all
    echo "🔧 Linting Rust code..."
    cargo clippy --all-targets --all-features --fix --allow-dirty --allow-staged
    echo "✅ All formatting and linting complete!"

# Client development commands
# Default: dev mode with release build and staging servers (recommended for most development)
client-dev:
    cd apps/client && pnpm tauri dev --release -- -- --set network.document_server=https://pod-server.ghost-spica.ts.net/server-staging --set network.identity_server=https://pod-server.ghost-spica.ts.net/identity-staging --set database.name=staging.db --set logging.level="debug"

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

# Release management
release VERSION:
    #!/bin/bash
    set -e
    echo "🚀 Starting release process for version {{VERSION}}..."
    
    # Check for clean working directory
    if [[ -n $(git status --porcelain) ]]; then
        echo "❌ Error: You have unstaged changes. Please commit or stash them before releasing."
        echo ""
        echo "Unstaged changes:"
        git status --short
        exit 1
    fi
    
    # Run version bump script
    ./scripts/bump-version.sh {{VERSION}}
    
    # Stage all changes
    git add -A
    
    # Commit version changes
    git commit -m "Bump version to {{VERSION}}"
    
    # Create git tag
    git tag v{{VERSION}}
    
    echo "✅ Release {{VERSION}} tagged successfully!"
    echo ""
    echo "Next steps:"
    echo "  just client-build  # Build the release"
    echo "  git push origin main --tags  # Push to remote"

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

# Generate JSON schemas
generate-schemas:
    cargo run --release -p pod-jsonschema > packages/pod2js/src/schemas.json

update-ts-types:
    # generate schemas
    just generate-schemas
    # update ts types
    cd packages/pod2js && pnpm gen-types
