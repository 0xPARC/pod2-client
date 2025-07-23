# PodNet Deployment Configuration

PodNet uses environment variables for configuration to make deployment easy across different platforms.

## Server Configuration

### Environment Variables

- `PORT` - Port to run the server on (default: 3000)
  - Note: Most platforms like Render automatically set this
- `PODNET_HOST` - Host to bind to (default: "0.0.0.0")
- `PODNET_MOCK_PROOFS` - Use mock proofs for faster development (default: true)
- `PODNET_DATABASE_PATH` - Path to SQLite database file (default: "app.db")
- `PODNET_CONTENT_STORAGE_PATH` - Path to content storage directory (default: "content")

### Examples

#### Development
```bash
# Use defaults (mock proofs enabled, port 3000)
cargo run -p podnet-server
```

#### Production on Render
Set these environment variables in your Render service:
```
PODNET_MOCK_PROOFS=false
PODNET_DATABASE_PATH=/opt/render/project/src/data/app.db
PODNET_CONTENT_STORAGE_PATH=/opt/render/project/src/data/content
```

The `PORT` environment variable will be automatically set by Render.

**Note**: You may want to use Render's persistent disk feature or an external database for production data storage.

## CLI Configuration

### Environment Variables

- `PODNET_SERVER_URL` - Main server URL (default: "http://localhost:3000")
- `PODNET_IDENTITY_SERVER_URL` - Identity server URL (default: "http://localhost:3001")

### Examples

#### Development
```bash
# Use defaults (localhost URLs)
cargo run -p podnet-cli -- --help
```

#### Production
```bash
export PODNET_SERVER_URL="https://your-podnet-server.onrender.com"
export PODNET_IDENTITY_SERVER_URL="https://your-identity-server.onrender.com"
cargo run -p podnet-cli -- list-posts
```

## Identity Server Configuration

The identity server uses environment variables for configuration:

### Core Configuration
- `PORT` - Port to run on (default: 3001)
- `PODNET_SERVER_URL` - Main server URL for registration (default: "http://localhost:3000")

### Keypair Storage
- `IDENTITY_KEYPAIR_FILE` - Path to keypair file (default: "identity-server-keypair.json")

### Examples

#### Development (Local Storage)
```bash
# Use defaults (local file storage)
cargo run -p podnet-ident-strawman
```

#### Production on Render with Secret Files
1. Create a secret file in your Render service containing the keypair JSON
2. Set the environment variable to point to the secret file:
```
IDENTITY_KEYPAIR_FILE=/etc/secrets/identity-keypair.json
PODNET_SERVER_URL=https://your-podnet-server.onrender.com
```

The identity server will automatically create and store a keypair on first startup if the file doesn't exist.

## Quick Setup for Render

1. Create two Render services (one for main server, one for identity server)
2. Set environment variables:
   - Main server: `PODNET_MOCK_PROOFS=false`
   - Identity server: (no special config needed)
3. Update your CLI environment:
   ```bash
   export PODNET_SERVER_URL="https://your-podnet-server.onrender.com"
   export PODNET_IDENTITY_SERVER_URL="https://your-identity-server.onrender.com"
   ```

That's it! No config files needed.