#!/bin/sh

cd apps/client
VITE_DOCUMENT_SERVER_URL=https://pod-server.ghost-spica.ts.net/server VITE_IDENTITY_SERVER_URL=https://pod-server.ghost-spica.ts.net/identity  RUST_LOG=trace pnpm tauri dev --release
