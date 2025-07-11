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