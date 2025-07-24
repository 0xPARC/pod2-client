# GitHub OAuth Identity Server

A POD2 identity server that uses GitHub OAuth for verification, providing enhanced identity PODs with GitHub-verified information.

## Features

- **GitHub OAuth Integration**: Users authenticate via GitHub OAuth flow
- **SSH Key Verification**: Fetches and includes user's GitHub SSH public keys
- **Enhanced Identity PODs**: Identity pods include GitHub username, user ID, and SSH keys
- **Backward Compatibility**: Maintains same API endpoints as strawman identity server
- **Automatic Registration**: Self-registers with podnet-server

## Environment Variables

Required:
- `GITHUB_CLIENT_ID`: GitHub OAuth app client ID
- `GITHUB_CLIENT_SECRET`: GitHub OAuth app client secret  
- `GITHUB_REDIRECT_URI`: OAuth callback URL (e.g., `http://localhost:3001/auth/github/callback`)

Optional:
- `IDENTITY_KEYPAIR_FILE`: Path to server keypair file (default: `github-identity-server-keypair.json`)
- `IDENTITY_DATABASE_PATH`: Path to SQLite database (default: `github-identity-users.db`)
- `PODNET_SERVER_URL`: PodNet server URL for registration (default: `http://localhost:3000`)

## OAuth Flow

1. **POST /auth/github**: Client provides public key and full name, gets GitHub authorization URL
2. **User visits GitHub**: User authenticates with GitHub and authorizes the app
3. **GET /auth/github/callback**: GitHub redirects back with authorization code
4. **POST /identity**: Client submits code and user info, server issues identity POD

## Identity POD Structure

```json
{
  "username": "User's Full Name",
  "github_username": "github_login", 
  "user_public_key": "...",
  "identity_server_id": "github-identity-server",
  "github_user_id": 12345,
  "github_public_keys": "[\"ssh-ed25519 AAAA...\", \"ssh-rsa AAAA...\"]",
  "github_email": "user@example.com",
  "oauth_verified_at": "2025-01-23T10:30:00Z",
  "issued_at": "2025-01-23T10:30:00Z",
  "_signer": "identity_server_public_key"
}
```

## Running

```bash
# Set environment variables
export GITHUB_CLIENT_ID="your_client_id"
export GITHUB_CLIENT_SECRET="your_client_secret" 
export GITHUB_REDIRECT_URI="http://localhost:3001/auth/github/callback"

# Run the server
cargo run -p identity-github
```

## GitHub OAuth App Setup

1. Go to GitHub Settings > Developer settings > OAuth Apps
2. Create a new OAuth app with:
   - Application name: "POD2 Identity Server"
   - Homepage URL: `http://localhost:3001`
   - Authorization callback URL: `http://localhost:3001/auth/github/callback`
3. Copy the Client ID and Client Secret to your environment variables

## API Endpoints

- `GET /` - Server info and public key
- `POST /auth/github` - Get GitHub OAuth authorization URL
- `GET /auth/github/callback` - Handle OAuth callback (redirects)
- `POST /identity` - Complete verification and issue identity POD
- `GET /lookup?public_key=...` - Username lookup (compatibility)