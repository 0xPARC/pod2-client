# GitHub OAuth Identity Integration

This document describes the GitHub OAuth identity integration for the POD2 client, which provides enhanced identity verification using GitHub's OAuth flow.

## Overview

The POD2 client now supports two identity server types:
1. **Standard Identity Server** (`strawman-identity-server`) - Simple challenge-response authentication
2. **GitHub OAuth Identity Server** (`github-identity-server`) - Enhanced GitHub OAuth verification

The client automatically detects which type of identity server it's connecting to and provides the appropriate UI flow.

## Components

### Frontend Components

#### 1. GitHubIdentitySetupModal (`src/components/GitHubIdentitySetupModal.tsx`)
Enhanced identity setup modal that provides a GitHub OAuth-specific flow:
- **Step 1**: Server detection and connection
- **Step 2**: GitHub OAuth initiation (full name entry)
- **Step 3**: OAuth callback handling (authorization code entry)
- **Step 4**: Setup completion with GitHub identity display

#### 2. GitHub OAuth Service (`src/lib/github-oauth.ts`)
TypeScript service layer for GitHub OAuth operations:
- Server detection (`setupGitHubIdentityServer`)
- Authorization URL generation
- OAuth completion handling
- GitHub identity POD parsing

### Backend Commands

#### 1. Tauri Commands (`src-tauri/src/features/identity_setup/github_oauth.rs`)

**`get_github_auth_url`**
```rust
async fn get_github_auth_url(
    server_url: String,
    public_key: serde_json::Value,
    username: String,
) -> Result<GitHubAuthUrlResponse, String>
```
Generates GitHub OAuth authorization URL via the identity server.

**`complete_github_identity_verification`**
```rust
async fn complete_github_identity_verification(
    server_url: String,
    code: String,
    state: String,
    username: String,
) -> Result<GitHubIdentityPodResult, String>
```
Completes OAuth flow and stores the GitHub identity POD.

**`detect_github_oauth_server`**
```rust
async fn detect_github_oauth_server(
    server_url: String,
) -> Result<bool, String>
```
Detects if a server is a GitHub OAuth identity server.

## User Flow

### GitHub OAuth Setup Flow

1. **Server Connection**: User enters identity server URL
2. **Server Detection**: Client detects GitHub OAuth server (`github-identity-server`)
3. **User Information**: User enters their full name (display name)
4. **OAuth Authorization**: 
   - Client requests authorization URL from server
   - Browser opens to GitHub OAuth page
   - User authorizes the POD2 application
5. **Callback Handling**:
   - User receives authorization code from GitHub
   - User pastes code back into the client
   - Client completes verification with server
6. **Identity POD Creation**:
   - Server creates enhanced identity POD with GitHub data
   - POD stored in client's "identity" folder
   - Setup marked as complete

### Enhanced Identity POD Structure

GitHub OAuth identity PODs contain additional fields:

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

## Configuration

### Client Configuration

The client uses the same network configuration as the standard identity setup:

```toml
[network]
identity_server = "http://localhost:3001"  # GitHub OAuth server URL
```

### GitHub OAuth Server Setup

The GitHub OAuth identity server requires these environment variables:

```bash
GITHUB_CLIENT_ID="your_github_app_client_id"
GITHUB_CLIENT_SECRET="your_github_app_client_secret"
GITHUB_REDIRECT_URI="http://localhost:3001/auth/github/callback"
```

### GitHub App Configuration

Create a GitHub OAuth app with:
- **Application name**: "POD2 Identity Server"
- **Homepage URL**: `http://localhost:3001`
- **Authorization callback URL**: `http://localhost:3001/auth/github/callback`

## Benefits

### Enhanced Security
- **OAuth Verification**: Users authenticate directly with GitHub
- **SSH Key Inclusion**: GitHub SSH keys included in identity POD
- **Unique GitHub ID**: Prevents duplicate accounts via GitHub user ID

### Rich Identity Data
- **GitHub Username**: Verified GitHub login
- **SSH Public Keys**: All user's GitHub SSH keys
- **Email Address**: GitHub-verified email (if public)
- **OAuth Timestamp**: When GitHub verification occurred

### User Experience
- **Familiar Flow**: Standard OAuth flow users know
- **Browser Integration**: Opens GitHub in user's default browser
- **Rich Profile**: GitHub profile data enhances identity verification

## Integration Points

### Backward Compatibility
- Works with existing POD verification predicates
- Compatible with PodNet publish/upvote flows
- Maintains same identity POD structure (with enhancements)

### Server Registration
- GitHub OAuth server self-registers with PodNet server
- Uses same challenge-response registration flow
- Provides enhanced server identification

## Development

### Running the GitHub OAuth Identity Server

```bash
# Set environment variables
export GITHUB_CLIENT_ID="your_client_id"
export GITHUB_CLIENT_SECRET="your_client_secret"
export GITHUB_REDIRECT_URI="http://localhost:3001/auth/github/callback"

# Run the server
cargo run -p identity-github
```

### Client Development

The client automatically detects GitHub OAuth servers and shows the enhanced UI. No special configuration needed beyond pointing to the GitHub OAuth server URL.

### Testing

1. **Server Detection**: Verify client detects GitHub OAuth servers
2. **OAuth Flow**: Test complete GitHub OAuth authorization flow
3. **Identity POD**: Verify enhanced identity POD creation and storage
4. **Backward Compatibility**: Test with both server types

## Security Considerations

### OAuth Security
- **State Parameter**: CSRF protection via cryptographic state
- **Authorization Code**: Single-use authorization codes
- **Server Validation**: Server validates OAuth tokens with GitHub

### Identity Verification
- **GitHub Verification**: Direct GitHub authentication
- **Public Key Binding**: User's cryptographic key bound to GitHub identity
- **SSH Key Verification**: GitHub SSH keys provide additional verification

### Data Privacy
- **Minimal Data**: Only necessary GitHub data included
- **User Consent**: Standard OAuth consent flow
- **Local Storage**: Identity PODs stored locally, not on server