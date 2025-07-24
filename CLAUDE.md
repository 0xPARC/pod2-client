# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository Overview

This is a unified POD2 ecosystem monorepo containing two main projects plus shared core libraries:

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

## Project Structure

```
core/                           # Shared libraries across all projects
├── models/                     # POD data models & verification predicates  
├── utils/                      # Common utility functions
└── solver/                     # Datalog query engine for POD requests

podnet/                         # PodNet publishing platform
├── server/                     # Content server (port 3000)
├── cli/                        # Command-line publishing client  
└── identity-strawman/          # Identity verification service (port 3001)

pod2-client/                    # POD2 client tools
├── db/                         # Client-specific database layer
└── apps/
    └── client/                 # Tauri desktop application

apps/                           # User-facing applications  
└── client/                     # Primary POD2 desktop client

packages/                       # JavaScript/TypeScript ecosystem
└── pod2-node/                  # Node.js native packages
```

## Projects

### 1. PodNet - Cryptographically-Secured Content Publishing Platform

PodNet is a trustless document publishing platform that uses zero-knowledge proofs to verify content authenticity and upvotes without revealing user identities.

**Key Components:**
- **PodNet Server** - REST API for content management with cryptographic verification
- **Identity Server** - Issues cryptographic identity pods after challenge-response flow
- **CLI Client** - User interface for publishing, upvoting, and identity management
- **Shared Models** - POD predicates for publish/upvote verification and counting

**Cryptographic Architecture:**
- **Publish Verification MainPod** - Proves document authenticity 
- **Upvote Verification MainPod** - Proves upvote authenticity
- **Upvote Count MainPod** - Proves correct upvote counts using recursion

### 2. POD2 Client Tools - Desktop Application & Development Tools

POD2 Client provides user-facing tools for managing personal POD collections, creating proofs, and participating in the POD2 ecosystem.

**Key Components:**
- **Tauri Desktop App** - Primary user interface for POD management
- **Database Layer** - SQLite-based POD storage with collection management
- **P2P Communication** - Distributed messaging between POD2 users

## Development Commands

### Quick Start Commands
```bash
# Install all dependencies
pnpm install

# Build and validate all code (recommended for most changes)
pnpm build                              # Builds all JS/TS with type checking
cargo build                             # Builds all Rust code

# Format all code  
just format
```

### Running Applications

**PodNet Platform:**
```bash
# Run PodNet content server (port 3000)
just podnet-server                      # or cargo run -p podnet-server

# Run identity verification service (port 3001)
just podnet-identity                     # or cargo run -p podnet-ident-strawman

# Use PodNet CLI
just podnet-cli --help                   # or cargo run -p podnet-cli -- --help
just podnet-cli publish document.md     # Publish a document
just podnet-cli upvote <document-id>     # Upvote a document
```

**POD2 Client Application:**
```bash
# Run desktop client in development mode
just client-dev                         # or cd apps/client && pnpm tauri dev

# Build desktop client for distribution
just client-build                       # or cd apps/client && pnpm tauri build
```

### Development & Testing Workflows

**For JavaScript/TypeScript development:**
```bash
pnpm build                              # Primary validation method
pnpm lint                               # Code style checking
pnpm test                               # Run test suites (when available)
```

**For Rust development:**
```bash
cargo clippy                            # Code analysis and linting
cargo test                              # Run test suites
cargo test -p <specific-crate>          # Test specific crate
```

## Key Technical Details

### Cryptographic Architecture
- Uses `pod2` library for zero-knowledge proof generation
- **SignedPODs** establish foundational trusted data
- **MainPODs** prove logical relationships between data privately
- Supports recursive proof chains and complex verification predicates

### Solver Engine  
- Implements semi-naive Datalog evaluation with proof reconstruction
- Uses metrics collection system with multiple levels (None, Counters, Debug, Trace)
- Supports query planning and optimization
- Materializes facts from POD collections for efficient querying

### Database Layers
**PodNet Database** (content publishing):
- Posts, documents, upvotes, and identity provider metadata
- Content-addressed storage using Poseidon hashes
- SQLite with migrations for metadata

**POD2 Client Database** (personal collection):  
- User's POD collections organized into spaces/workspaces
- Private key storage for signing operations
- P2P messaging and chat functionality
- SQLite with deadpool connection pooling

### Tauri Frontend-Backend Communication

**Type-Safe API Wrapper Pattern:**
The POD2 Client uses a consistent pattern for type-safe communication between the TypeScript frontend and Rust Tauri backend:

1. **Backend Commands** - Rust functions marked with `#[tauri::command]` in `src-tauri/src/features/`
2. **TypeScript Types** - Shared interfaces in `apps/client/src/lib/documentApi.ts`
3. **Wrapper Functions** - Type-safe wrapper functions that handle `invoke()` calls and error handling

**Example Pattern:**
```typescript
// TypeScript interface matching Rust struct
export interface DraftRequest {
  title: string;
  content_type: string;
  message: string | null;
  // ... other fields
}

// Type-safe wrapper function
export async function createDraft(request: DraftRequest): Promise<string> {
  try {
    return await invoke<string>("create_draft", { request });
  } catch (error) {
    throw new Error(`Failed to create draft: ${error}`);
  }
}
```

**Benefits:**
- Compile-time type checking between frontend and backend
- Centralized error handling with consistent error messages
- Easy refactoring and maintenance of IPC communication
- Clear separation between business logic and IPC details

**Usage Guidelines:**
- Always use wrapper functions instead of direct `invoke()` calls in components
- Define TypeScript interfaces that match Rust command parameter and return types
- Handle errors consistently in wrapper functions with descriptive messages
- Keep wrapper functions in dedicated API modules (e.g., `documentApi.ts`)

### Development Configuration

**Mock vs Real Proofs:**
- `mock_proofs = true` - Fast mock proofs for development (PodNet)
- `mock_proofs = false` - Real ZK proofs for production
- Environment variables override config with `PODNET_` prefix

**Feature Flags** (POD2 Client):
- `FEATURE_POD_MANAGEMENT=true|false` - Core collection management
- `FEATURE_NETWORKING=true|false` - P2P communication  
- `FEATURE_AUTHORING=true|false` - Creating and signing PODs
- `FEATURE_INTEGRATION=true|false` - External POD request handling

## Code Style Guidelines

1. **Prefer small, modular, composable functions** - Functions should do one thing well
2. **Parse, don't validate** - Transform inputs into type-safe representations early  
3. **No unhelpful comments** - Avoid comments that describe what code does rather than why
4. **Prefer direct imports** - Import specific items rather than entire modules
5. **Use composable functions** - Break down complex operations into smaller, focused functions

### Rust-Specific Guidelines
6. **Avoid compiler warnings** - Keep codebase warning-free, use `#[allow(dead_code)]` sparingly
7. **Explicit imports over wildcards** - Prefer `use tauri::{State, Manager}` over `use tauri::*`
8. **Module documentation over marker structs** - Use `//!` module comments instead of empty docs
9. **Clean re-exports** - Only re-export items that are actually consumed elsewhere

## Testing & Validation

### Frontend/TypeScript Code Changes

**When updating client app or any JavaScript/TypeScript code:**
```bash
# From repository root - builds and type-checks all JS/TS code
pnpm build
```

This uses Turborepo to build and type-check all JavaScript/TypeScript components across the monorepo. This is the **primary validation method** for frontend changes.

**Important:** Do not try to run the client app for testing. If application functionality requires testing, ask the user to run the app and test it themselves.

### Rust Code Changes

**For changes in a specific crate:**
```bash
# Run in the specific crate directory
cargo clippy
```

**For wide-ranging Rust changes:**
```bash
# From repository root
cargo clippy
```

**Running tests (when needed):**
```bash
# All Rust tests
cargo test

# Specific project tests  
cargo test -p podnet-server
cargo test -p podnet-models  
cargo test -p pod2_solver
cargo test -p pod2_db
```

## Configuration Files

- **Root**: `Cargo.toml` (workspace configuration), `rust-toolchain.toml`, `justfile`
- **PodNet**: `podnet/server/config.toml` (mock proofs, database settings)
- **POD2 Client**: Feature flags via environment variables
- **Web**: `package.json`, `pnpm-workspace.yaml`, `turbo.json`

## Documentation

- **API Documentation**: `docs/API_DOCUMENTATION.md` (PodNet REST APIs)
- **Deployment**: `docs/DEPLOYMENT.md` (production deployment guide)
- **README**: Root `README.md` with ecosystem overview

## Key File Locations

- **PodNet Server**: `podnet/server/src/main.rs`
- **PodNet CLI**: `podnet/cli/src/main.rs`  
- **Identity Server**: `podnet/identity-strawman/src/main.rs`
- **POD2 Desktop App**: `apps/client/src-tauri/src/main.rs`
- **Solver Engine**: `core/solver/src/lib.rs` 
- **Shared Models**: `core/models/src/lib.rs`
- **Database Migrations**: `podnet/server/migrations/`, `pod2-client/db/migrations/`

## Environment Variables

- `RUST_LOG` - Controls logging level (info, debug, trace, etc.)
- `PODNET_MOCK_PROOFS` - Override proof mode for PodNet server
- `FEATURE_*` - Feature flags for POD2 client application

## Integration Between Projects

- **PodNet** publishes content that can be imported into **POD2 Client** collections
- **POD2 Client** can create MainPODs that prove statements about PodNet-published content
- Both projects share **core libraries** (models, solver, utilities) for consistency
- **Solver engine** processes POD requests from both PodNet upvote verification and POD2 Client queries

This unified ecosystem enables users to participate in cryptographically-verified content publishing (PodNet) while managing their personal POD collections and creating custom proofs (POD2 Client).