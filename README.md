# Podnet Client and Server

This monorepo contains the complete Podnet ecosystem, including a desktop client application, publishing platform, and servers.

## Repository Structure

This monorepo is organized into several key components:

```
core/                           # Shared libraries across all projects
├── models/                     # POD data models & verification predicates  
├── utils/                      # Common utility functions
└── solver/                     # Datalog query engine for POD requests

podnet/                         # PodNet publishing platform
├── server/                     # Content server (default port: 3000)
├── cli/                        # Command-line publishing client  
└── identity-strawman/          # Identity verification service (default port: 3001)

apps/client/                    # Primary POD2 desktop application
├── src-tauri/                  # Rust backend (Tauri)
└── src/                        # React frontend

pod2-client/                    # POD2 client tools
└── db/                         # Client-specific database layer

packages/                       # JavaScript/TypeScript ecosystem
└── pod2-node/                  # Node.js native packages

docs/                           # Documentation
├── API_DOCUMENTATION.md        # PodNet API reference
└── DEPLOYMENT.md               # Production deployment guide
```

### Component Descriptions

- **Core Libraries**: Shared Rust libraries including the Datalog solver engine, POD data models, and common utilities
- **Podnet Platform**: Cryptographically-secured content publishing platform with server, CLI, and identity services
- **Desktop Client**: Tauri-based desktop application for managing personal POD collections and P2P communication
- **Web Packages**: TypeScript/JavaScript packages for web integration

## Prerequisites

- **Rust and Cargo** (latest stable version)
- **Node.js** (v18 or later)
- **pnpm** (for JavaScript/TypeScript package management)
- **just** (command runner - install with `cargo install just`)

## Quick Start

1. **Clone and setup:**
   ```bash
   git clone <repository-url>
   cd client-pod2
   pnpm install
   ```

2. **Start development (recommended):**
   ```bash
   just client-dev
   ```
   This runs the desktop client with staging servers and a local database.

3. **For full local development:**
   ```bash
   just dev-local
   ```
   This starts local servers and the desktop client.

## Development Commands

The repository uses [just](https://github.com/casey/just) as a command runner. All available commands are defined in the `justfile`.

### Quick Start Commands

| Command | Description |
|---------|-------------|
| `just client-dev` | **Main development command** - runs client with staging servers (recommended) |
| `just dev-local` | Full local development environment (client + local servers) |
| `just dev-all` | All services using mprocs (requires `pnpm install -g mprocs`) |

### Client Development

| Command | Description |
|---------|-------------|
| `just client-dev` | Client with staging servers, release build (default) |
| `just client-dev-debug` | Client with debug build (slower, better for debugging) |
| `just client-dev-local` | Client with local servers (requires `just servers-local`) |
| `just client-dev-prod` | Client with production servers (testing against prod) |
| `just client-build` | Build client for production distribution |

### Server Management

| Command | Description |
|---------|-------------|
| `just servers-local` | Run both document and identity servers locally |
| `just podnet-server` | Run PodNet document server only (port 3000) |
| `just podnet-identity` | Run identity verification service only (port 3001) |

### PodNet CLI

| Command | Description |
|---------|-------------|
| `just podnet-cli [args]` | Run PodNet command-line interface |
| `just podnet-cli --help` | Show available CLI commands |
| `just podnet-cli publish document.md` | Example: publish a document |
| `just podnet-cli upvote <document-id>` | Example: upvote a document |

### Development Environments

| Command | Description |
|---------|-------------|
| `just dev-local` | Client + local servers (full local development) |
| `just dev-all` | All services using mprocs process manager |
| `just dev-services` | Just backend services (no desktop app) |

### Build & Code Quality

| Command | Description |
|---------|-------------|
| `just js-build` | Build all JavaScript/TypeScript components |
| `just format` | Format all code (JavaScript + Rust) |
| `just build-all` | Build entire Rust workspace |

### CI/Testing

| Command | Description |
|---------|-------------|
| `just ci-all` | **Run all CI checks locally** (same as GitHub workflows) |
| `just ci-rust` | Rust-specific checks (fmt, clippy, tests) |
| `just ci-js` | JavaScript/TypeScript checks (lint, format, test, build) |
| `just check-quick` | Fast development checks (fmt, clippy, lint) |

## Projects Overview

### 1. Podnet - POD-Based Content Publishing Platform

A trustless document publishing platform that uses zero-knowledge proofs to verify content authenticity and upvotes without revealing user identities.

**Components:**
- **Podnet Server**: REST API for content management with cryptographic verification
- **Identity Server**: Issues cryptographic identity pods after challenge-response flow  
- **CLI Client**: Command-line interface for publishing, upvoting, and identity management
- **Shared Models**: POD predicates for publish/upvote verification and counting

**Cryptographic Features:**
- Publish verification using MainPODs to prove document authenticity
- Upvote verification using MainPODs to prove upvote authenticity  
- Upvote counting using recursive MainPODs for accurate tallying

### 2. Podnet Desktop Client

A Tauri-based desktop application providing user-friendly tools for managing personal POD collections, creating proofs, and participating in the POD2 ecosystem.

**Features:**
- **POD Management**: Import, export, and organize POD collections
- **Proof Creation**: User-friendly interface for creating MainPODs
- **Identity Integration**: Connect with PodNet identity services
- **Development Tools**: Code editor and debugging tools for POD development

### 3. Core Libraries

Shared Rust libraries that power the entire ecosystem:

- **Solver Engine**: Semi-naive Datalog evaluation with proof reconstruction
- **POD Models**: Data structures and verification predicates
- **Utilities**: Common functions for cryptographic operations and data handling

## Getting Started for New Developers

1. **Set up your environment:**
   ```bash
   # Install prerequisites
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh  # Rust
   npm install -g pnpm                                            # pnpm
   cargo install just                                             # just
   
   # Clone and setup
   git clone <repository-url>
   cd client-pod2
   pnpm install
   ```

2. **Try the desktop client:**
   ```bash
   just client-dev
   ```
   This starts the desktop app connected to staging servers with a local database.

3. **Explore PodNet CLI:**
   ```bash
   just podnet-cli --help
   just servers-local  # Start local servers in another terminal
   just podnet-cli publish README.md  # Publish this README as a test
   ```

4. **Development workflow:**
   - Use `just client-dev` for day-to-day client development
   - Use `just dev-local` when you need to test against local servers
   - Use `just format` before committing code
   - Use `just ci-all` to run the same checks as CI

5. **Debugging:**
   - Use `just client-dev-debug` for debug builds with better error messages
   - Check logs in the desktop app's developer tools
   - Server logs appear in the terminal when running local servers

## Configuration

The desktop client supports configuration through:
- **Config files**: TOML files in the app's config directory  
- **Environment variables**: `POD2_CONFIG_FILE` to specify config file location
- **CLI overrides**: `--set key.path=value` to override any config setting

Example:
```bash
just client-dev -- --set database.path=./custom.db --set network.document_server=http://localhost:3000
```

## Contributing

1. **Code style**: Run `just format` before committing
2. **Testing**: Run `just ci-all` to ensure all checks pass
3. **Documentation**: Update relevant documentation when adding features
4. **Database changes**: Add migrations in `pod2-client/db/migrations/`

## Documentation

- **API Documentation**: See `docs/API_DOCUMENTATION.md` for PodNet REST API reference
- **Deployment Guide**: See `docs/DEPLOYMENT.md` for production deployment instructions  
- **Development Details**: See `CLAUDE.md` for detailed technical documentation
