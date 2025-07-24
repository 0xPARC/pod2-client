#!/bin/sh

if ! command -v pnpm &> /dev/null; then
    echo "pnpm could not be found, installing..."
    npm install -g pnpm
fi

pnpm install

cd apps/client

if ! command -v just &> /dev/null; then
    echo "just could not be found, installing..."
    cargo install just
fi

just client-dev