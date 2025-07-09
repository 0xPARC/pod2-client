# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

POD2 Client Tools and Experiments is a multi-component system for working with POD2 (Provable Object Data) cryptographic primitives. This repository contains experimental client tools and applications that enable users to create, manage, and query cryptographic data containers with zero-knowledge proof capabilities.

### What is POD2?

POD2 (Provable Object Data) is a cryptographic system for creating **verifiable statements about data** while maintaining privacy through zero-knowledge proofs. At its core, POD2 enables users to make claims about data they possess and prove those claims cryptographically without revealing the underlying data.

#### Core Components

**PODs (Provable Object Data containers)** are cryptographic data structures that come in two main types:

- **SignedPODs**: Cryptographically signed key-value stores that establish authoritative data
  - Contains a Merkle tree of key-value pairs with digital signatures
  - Used for foundational data like government IDs, certificates, attestations
  - Provides the base layer of trusted information

- **MainPODs**: Zero-knowledge proofs that derive new statements from existing PODs
  - Proves logical relationships between data without revealing the data itself
  - Can combine multiple input PODs to create new claims
  - Enables complex reasoning while preserving privacy

**Statements** are assertions about data relationships using built-in predicates like `Equal`, `Lt`, `Contains`, and arithmetic operations (`SumOf`, `ProductOf`, etc.). Custom predicates can be defined using logical combinations.

**Operations** are the mechanisms for deriving new statements, such as copying statements between PODs, proving equality through value comparison, or demonstrating membership with Merkle proofs.

**Anchored Keys** provide indirect references to data using the format `pod_id["key"]`, enabling statements about data without directly exposing it.

#### Key Capabilities

- **Privacy-preserving proofs**: Verify claims about data without revealing the data
- **Composable logic**: Combine multiple PODs to create complex proofs  
- **Cryptographic integrity**: All claims are backed by digital signatures or zero-knowledge proofs
- **Flexible data types**: Support for integers, strings, booleans, arrays, dictionaries, and sets
- **Recursive verification**: MainPODs can reference other PODs to build proof chains

#### Architecture

The system uses a clean **frontend/middleware/backend architecture**:
- **Frontend**: User-friendly APIs for POD creation and management
- **Middleware**: Typed interfaces for statements and operations  
- **Backend**: Cryptographic proof generation using zero-knowledge circuits

A **Datalog-based solver** handles complex queries and proof requests, enabling recursive logic and distributed problem-solving across multiple PODs and parties.

### What This Repository Builds

This repository develops a comprehensive set of **POD2 client tools** designed to make POD2 technology accessible and practical for real-world use. The project consists of three main categories:

#### 1. POD2 Client Application (Primary Focus)
A desktop application built with Tauri that serves as a complete POD2 client for end-users, featuring:

- **POD Collection Management**: Full UI for browsing, organizing, and managing a local collection of PODs
- **POD Creation**: Tools for creating new SignedPODs and MainPODs through an intuitive interface
- **POD Request Handling**: Ability to respond to external POD Requests received through:
  - Custom URL schemes (deep linking)
  - Network requests from other applications
  - File-based requests
- **POD Request Creation**: Tools for composing and sending POD Requests to other users
- **Cryptographic Key Management**: Secure creation, storage, and management of private keys
- **P2P Communication Network**: Distributed communication system for exchanging PODs and POD Requests between users

#### 2. Ecosystem Libraries and Packages
Core libraries that power the client app but are designed for broader ecosystem use:

**Rust Libraries:**
- **Database Layer**: SQLite-based POD storage with migrations and connection pooling
- **Solver Engine**: Datalog query processor for evaluating POD Requests
- **Server Components**: HTTP API framework for POD operations and management

**JavaScript/TypeScript Packages:**
- **POD2 Client Libraries**: JavaScript/TypeScript packages for POD creation, verification, and management
- **JSON Schemas**: Standardized schemas for POD serialization and interchange
- **Node.js Native Packages**: High-performance native modules for cryptographic operations
- **Web Components**: Reusable UI components for POD-enabled web applications
- **API Bindings**: TypeScript interfaces for POD2 server APIs and network protocols

#### 3. Experimental and Demonstration Tools
Development and research tools including:

- **Playground Client**: Web-based developer sandbox for experimenting with POD operations
- **Sample Applications**: Reference implementations demonstrating POD2 capabilities
- **Testing Infrastructure**: Tools for validating POD2 functionality across different scenarios

The client application represents the primary user-facing interface to the POD2 ecosystem, enabling individuals to participate in privacy-preserving data exchange and verification workflows without requiring deep cryptographic expertise.

## Quick Start

The main entry point is the playground script:
```bash
./playground.sh
```

This runs the full development environment with all components.

## Architecture

The repository is organized as a multi-crate Rust workspace with web frontend components:

### Core Components

- **`server/`** - DEPRECATED HTTP API server providing POD2 operations and local POD collection management
- **`solver/`** - Datalog engine for POD Request queries, implements semi-naive evaluation
- **`db/`** - Database abstraction layer with SQLite connection pooling and migrations
- **`apps/playground-client/`** - DEPRECATED React developer sandbox for POD management and operations (functionality gradually being ported to the `client` app)
- **`apps/client/`** - Tauri-based desktop application (primary user-facing client)

### Key Relationships

1. **Server** uses **Solver** for query processing and **DB** for persistence
2. **Playground Client** communicates with **Server** via HTTP API (developer sandbox)
3. **Tauri App** is the primary user-facing client, intended for regular users
4. **Solver** implements the core POD2 Datalog engine with proof reconstruction
5. **DB** provides abstracted database operations with connection pooling

### Development Priorities

- **Tauri App** is the main focus for future development as the user-facing client
- **Playground Client** serves as a developer sandbox for experimentation
- Plans to factor out reusable components from playground for use in Tauri app
- Integration of Tauri app into TypeScript monorepo structure for code sharing

## Development Commands

### Rust Components
```bash
# Build all Rust components
cargo build

# Run server in development mode
RUST_LOG=info cargo run --release --bin pod2-server

# Run tests
cargo test

# Format code
cargo fmt

# Run linter
cargo clippy
```

### Web Components
```bash
pnpm install
pnpm dev          # Development mode
pnpm build        # Production build
pnpm test         # Run tests
pnpm lint         # Run oxlint
pnpm format       # Format with prettier
```

### Tauri App
```bash
# From apps/client/ directory
pnpm dev          # Development mode
pnpm build        # Production build
pnpm tauri        # Tauri CLI commands
pnpm dlx shadcn@latest # For handling shadcn components
```

## Key Technical Details

### Solver Architecture
- Implements semi-naive Datalog evaluation with proof reconstruction
- Uses metrics collection system with multiple levels (None, Counters, Debug)
- Supports query planning and optimization
- Materializes facts from POD collections

### Database Layer
- Uses deadpool-sqlite for connection pooling
- Implements rusqlite-migration for schema management
- Supports both file-based and in-memory databases
- Migrations are embedded using include_dir

### Server API
- Built with Axum web framework
- Provides REST API for POD operations
- Handles playground operations, POD management, and space management
- Integrates with solver for query processing

### Web Frontend
- React with TypeScript
- Uses Shadcn for components. Use `pnpm dlx shadcn@latest` to run shadcn commands.
- Support both light and dark mode, and Shadcn theming by leveraging the CSS variables and utility classes in App.css.
- Uses Vite for build tooling and development
- Turbo for monorepo management
- **Playground Client**: Implements IDE-like layout with resizable panes and Monaco editor
- **Tauri App**: User-facing desktop application with plans for component reuse from playground

## Code Style Guidelines

Based on the Cursor rules in `.cursor/rules/`:

1. **Prefer small, modular, composable functions** - Functions should do one thing well
2. **Parse, don't validate** - Transform inputs into type-safe representations early
3. **No unhelpful comments** - Avoid comments that describe what code does rather than why
4. **Prefer direct imports** - Import specific items rather than entire modules
5. **Use composable functions** - Break down complex operations into smaller, focused functions

## Configuration

### Rust Toolchain
- Uses specific toolchain defined in `rust-toolchain.toml`
- Formatting rules in `rustfmt.toml`
- Rust analyzer configuration in `rust-analyzer.toml`

### Dependencies
- POD2 library is sourced from GitHub (branch: playground-tweaks)
- Can be overridden to use local path via `.cargo/config.toml`
- Example configuration provided in `.cargo/config.toml.example`

## Testing

### Rust Tests
```bash
cargo test                    # All tests
cargo test -p pod2_solver     # Specific crate tests
cargo test -p pod2_server     # Server tests
cargo test -p pod2_db         # Database tests
```

### Web Tests
```bash
# From web/ directory
pnpm test                     # All web tests
pnpm test --filter playground-client  # Specific app tests
```

## Database

- SQLite database with migrations in `db/migrations/` and `server/migrations/`
- Database file: `pod2.db` (created automatically)
- Schema includes pods, spaces, and private key storage

## Key File Locations

- **Server entry point**: `server/src/bin/` (multiple binaries)
- **Solver entry point**: `solver/src/lib.rs` (main solve function)
- **Database migrations**: `db/migrations/` and `server/migrations/`
- **Playground app**: `apps/playground-client/src/App.tsx` (developer sandbox)
- **Tauri app**: `apps/client/src-tauri/src/main.rs` (primary user client)
- **Tauri frontend**: `apps/client/src/App.tsx` (React frontend for desktop app)

## Environment Variables

- `RUST_LOG` - Controls logging level (info, debug, trace, etc.)
- Database path can be configured via server configuration