# PodNet API Documentation

This document describes the API endpoints, request/response structures, and database schema for the PodNet system, which consists of three main components:

1. **PodNet Server** (port 3000) - Main content server
2. **Identity Server** (port 3001) - Identity verification service  
3. **CLI Client** - Command-line interface for users

## System Overview

PodNet is a content publishing platform using POD2 to verify:
- User identity through registered identity servers
- Document authenticity and authorship
- Upvote integrity and counting
- Content integrity through content-addressed storage

## PodNet Server API (Port 3000)

Note: The ultimate source of truth on the objects each route requests is in
`models/src/lib.rs`.

### Root Endpoint
- **GET** `/` - Server status and information

### Posts
- **GET** `/posts` - List all posts with their documents
- **GET** `/posts/:id` - Get specific post with documents

### Documents  
- **GET** `/documents` - List all document metadata
- **GET** `/documents/:id` - Get specific document with content
- **GET** `/documents/:id/replies` - Get documents that reply to this document
- **POST** `/publish` - Publish new document


### Identity Server Management
- **POST** `/identity/challenge` - Request challenge for identity server registration
- **POST** `/identity/register` - Register identity server with challenge response

### Upvotes
- **POST** `/documents/:id/upvote` - Upvote a document

## Identity Server API (Port 3001)

### Server Information
- **GET** `/` - Get identity server info (server_id, public_key)

### Identity Verification Flow
- **POST** `/user/challenge` - Request challenge for user identity verification  
- **POST** `/identity` - Submit challenge response, receive identity pod

## Request/Response Data Structures

### Document Publishing

#### POST `/publish`
**Request Body:**
```json
{
  "content": {
    "message": "# Document Content\nMarkdown content here...",
    "file": {
      "name": "example.png",
      "content": [/* file bytes */],
      "mime_type": "image/png"
    },
    "url": "https://example.com"
  },
  "tags": ["tag1", "tag2"],
  "authors": ["alice", "bob"],
  "reply_to": 42,
  "main_pod": { /* MainPod proving identity and document authenticity */ }
}
```

**Note:** The `content` field must contain at least one of `message`, `file`, or `url`. All three can be provided together. The `reply_to` field is optional and references another document's ID.

**MainPod Predicate:** `publish_verified(username, data, identity_server_pk)`

```
identity_verified(username, private: identity_pod) = AND(
    Equal(?identity_pod["_type"], SIGNED_POD)
    Equal(?identity_pod["username"], ?username)
)

publish_verified(username, data, identity_server_pk, private: identity_pod, document_pod) = AND(
    identity_verified(?username)
    Equal(?document_pod["request_type"], "publish")
    Equal(?document_pod["data"], ?data)
    Equal(?document_pod["_signer"], ?identity_pod["user_public_key"])
    Equal(?identity_pod["_signer"], ?identity_server_pk)
)
```

**Proves:**
- Identity pod was signed by registered identity server
- Document pod was signed by authenticated user
- Document signer matches identity user_public_key
- Document contains correct content hash and post_id

**Response:**
```json
{
  "metadata": {
    "id": 1,
    "content_id": "0x1234...", 
    "post_id": 1,
    "revision": 1,
    "created_at": "2024-01-01T00:00:00Z",
    "pod": { /* MainPod */ },
    "timestamp_pod": { /* Server timestamp pod */ },
    "uploader_id": "alice",
    "upvote_count": 0,
    "upvote_count_pod": null,
    "tags": ["tag1", "tag2"],
    "authors": ["alice", "bob"],
    "reply_to": 42
  },
  "content": {
    "message": "# Document Content\nMarkdown content here...",
    "file": {
      "name": "example.png",
      "content": [/* file bytes */],
      "mime_type": "image/png"
    },
    "url": "https://example.com"
  }
}
```

### Identity Server Registration

The identity server registration follows a secure challenge-response flow:

1. **Challenge Request**: Identity server requests a cryptographically signed challenge from main server
2. **Challenge Verification**: Identity server verifies the main server's challenge pod signature  
3. **Response Creation**: Identity server creates a signed response pod containing the challenge
4. **Registration Submission**: Both the server's challenge pod and identity server's response pod are submitted
5. **Full Verification**: Main server verifies both pods, challenge expiration, and cryptographic signatures

This ensures the main server controls challenge generation and the identity server proves control of their private key.

#### POST `/identity/challenge`
**Request Body:**
```json
{
  "server_id": "strawman-identity-server",
  "public_key": { /* Identity server's public key */ }
}
```

**Response:**
```json
{
  "challenge_pod": {
    "entries": {
      "challenge": "abcdef123456...",
      "expires_at": "2024-01-01T00:05:00Z",
      "identity_server_public_key": { /* Identity server's public key from request */ },
      "server_id": "strawman-identity-server",
      "_signer": { /* Main server's public key */ },
      "_type": SignedPod
    },
    "signature": { /* Main server's signature */ }
  }
}
```

#### POST `/identity/register`
**Request Body:**
```json
{
  "server_challenge_pod": {
    "entries": {
      "challenge": "abcdef123456...",
      "expires_at": "2024-01-01T00:05:00Z", 
      "identity_server_public_key": { /* Identity server's public key */ },
      "server_id": "strawman-identity-server",
      "_signer": { /* Main server's public key */ },
      "_type": SignedPod
    },
    "signature": { /* Main server's signature */ }
  },
  "identity_response_pod": {
    "entries": {
      "challenge": "abcdef123456...",
      "server_id": "strawman-identity-server",
      "_signer": { /* Identity server's public key */ },
      "_type": SignedPod
    },
    "signature": { /* Identity server's signature */ }
  }
}
```

**Response:**
```json
{
  "public_key": { /* Server's public key */ }
}
```

### Identity Verification Flow

#### POST `/user/challenge` (Identity Server)
**Request Body:**
```json
{
  "username": "alice",
  "user_public_key": { /* User's public key */ }
}
```

**Response:**
```json
{
  "challenge_pod": {
    "entries": {
      "challenge": "abcdef123456...",
      "expires_at": "2024-01-01T00:05:00Z",
      "user_public_key": { /* User's public key from request */ },
      "username": "alice",
      "_signer": { /* Identity server's public key */ },
      "_type": SignedPod
    },
    "signature": { /* Identity server's signature */ }
  }
}
```

#### POST `/identity` (Identity Server)
**Request Body:**
```json
{
  "server_challenge_pod": {
    "entries": {
      "challenge": "abcdef123456...",
      "expires_at": "2024-01-01T00:05:00Z",
      "user_public_key": { /* User's public key */ },
      "username": "alice",
      "_signer": { /* Identity server's public key */ },
      "_type": SignedPod
    },
    "signature": { /* Identity server's signature */ }
  },
  "user_response_pod": {
    "entries": {
      "challenge": "abcdef123456...",
      "username": "alice",
      "_signer": { /* User's public key */ },
      "_type": SignedPod
    },
    "signature": { /* User's signature */ }
  }
}
```

**Response:**
```json
{
  "identity_pod": {
    "entries": {
      "username": "alice",
      "user_public_key": { /* User's public key */ },
      "identity_server_id": "strawman-identity-server",
      "issued_at": "2024-01-01T00:00:00Z",
      "_signer": { /* Identity server's public key */ },
      "_type": 1
    },
    "signature": { /* Identity server's signature */ }
  }
}
```

### Upvoting

#### POST `/documents/:id/upvote`
**Request Body:**
```json
{
  "upvote_main_pod": { /* MainPod proving upvote authenticity */ },
}
```

**MainPod Predicate:** `upvote_verification(username, content_hash, identity_server_pk)`

```
identity_verified(username, private: identity_pod) = AND(
    Equal(?identity_pod["_type"], SIGNED_POD)
    Equal(?identity_pod["username"], ?username)
)

upvote_verified(content_hash, private: upvote_pod) = AND(
    Equal(?upvote_pod["_type"], SIGNED_POD)
    Equal(?upvote_pod["content_hash"], ?content_hash)
    Equal(?upvote_pod["request_type"], "upvote")
)

upvote_verification(username, content_hash, identity_server_pk, private: identity_pod, upvote_pod) = AND(
    identity_verified(?username)
    upvote_verified(?content_hash)
    Equal(?identity_pod["_signer"], ?identity_server_pk)
    Equal(?identity_pod["user_public_key"], ?upvote_pod["_signer"])
)
```

**Proves:**
- Identity pod was signed by registered identity server
- Upvote pod was signed by authenticated user
- Upvote signer matches identity user_public_key
- Upvote references correct document and post

**Response:**
```json
{
  "success": true,
  "upvote_id": 123,
  "document_id": 1,
  "upvote_count": 5
}
```

### Data Retrieval

#### GET `/posts`
**Response:**
```json
[
  {
    "id": 1,
    "created_at": "2024-01-01T00:00:00Z",
    "last_edited_at": "2024-01-01T00:00:00Z",
    "documents": [
      {
        "id": 1,
        "content_id": "0x1234...",
        "post_id": 1,
        "revision": 1,
        "created_at": "2024-01-01T00:00:00Z",
        "pod": { /* MainPod for the document verification */ },
        "timestamp_pod": { /* SignedPod */ },
        "uploader_id": "alice",
        "upvote_count": 5,
        "upvote_count_pod": { /* MainPod proving upvote count */ },
        "tags": ["tag1", "tag2"],
        "authors": ["alice", "bob"],
        "reply_to": null
      }
    ]
  }
]
```

**Upvote Count Pod Predicate:** `upvote_count(count, content_hash)`

```
use _, _, upvote_verification from [upvote_batch_id]

upvote_count_base(count, content_hash, private: data_pod) = AND(
    Equal(?count, 0)
    Equal(?data_pod["content_hash"], ?content_hash)
)

upvote_count_ind(count, content_hash, private: intermed, username, identity_server_pk) = AND(
    upvote_count(?intermed, ?content_hash)
    SumOf(?count, ?intermed, 1)
    upvote_verification(?username, ?content_hash, ?identity_server_pk)
)

upvote_count(count, content_hash) = OR(
    upvote_count_base(?count, ?content_hash)
    upvote_count_ind(?count, ?content_hash)
)
```

**Proves:**
- **Base case:** Document starts with 0 upvotes
- **Inductive case:** Count increments by 1 from previous proven count
- **Recursive verification:** Builds chain of proofs from 0 to current count

#### GET `/documents`
**Response:**
```json
[
  {
    "id": 1,
    "content_id": "0x1234...",
    "post_id": 1,
    "revision": 1,
    "created_at": "2024-01-01T00:00:00Z",
    "pod": { /* MainPod with verification */ },
    "timestamp_pod": { /* Server timestamp */ },
    "uploader_id": "alice",
    "upvote_count": 5,
    "upvote_count_pod": { /* Upvote count proof */ },
    "tags": ["tag1", "tag2"],
    "authors": ["alice", "bob"],
    "reply_to": null
  }
]
```

#### GET `/documents/:id/replies`
**Response:**
```json
[
  {
    "id": 2,
    "content_id": "0x5678...",
    "post_id": 1,
    "revision": 1,
    "created_at": "2024-01-01T01:00:00Z",
    "pod": { /* MainPod with verification */ },
    "timestamp_pod": { /* Server timestamp */ },
    "uploader_id": "bob",
    "upvote_count": 2,
    "upvote_count_pod": { /* Upvote count proof */ },
    "tags": ["reply"],
    "authors": ["bob"],
    "reply_to": 1
  }
]
```

## CLI Usage

The CLI now supports multi-content documents and replies:

### Publishing Content
```bash
# Publish with message only
podnet-cli publish --keypair keypair.json --identity-pod identity.json --message "Hello world"

# Publish with file only
podnet-cli publish --keypair keypair.json --identity-pod identity.json --file image.png

# Publish with URL only
podnet-cli publish --keypair keypair.json --identity-pod identity.json --url https://example.com

# Publish with all content types
podnet-cli publish --keypair keypair.json --identity-pod identity.json \
  --message "Check out this image from the website" \
  --file screenshot.png \
  --url https://example.com

# Publish with tags and authors
podnet-cli publish --keypair keypair.json --identity-pod identity.json \
  --message "Team update" \
  --tags "update,team,progress" \
  --authors "alice,bob,charlie"

# Reply to another document
podnet-cli publish --keypair keypair.json --identity-pod identity.json \
  --message "Great point!" \
  --reply-to 42
```

**Note:** At least one of `--message`, `--file`, or `--url` must be provided.

### Content Rendering
The CLI now performs client-side rendering with intelligent format detection:
- **Markdown**: Rendered using pulldown-cmark with math support
- **HTML**: Rendered as-is
- **Images**: Displayed as base64-encoded images
- **JSON**: Pretty-printed with syntax highlighting
- **Other content**: Displayed as plain text

## Database Schema

The PodNet server uses SQLite with the following tables:

### `posts`
```sql
CREATE TABLE posts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    last_edited_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

### `documents`
```sql
CREATE TABLE documents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    content_id TEXT NOT NULL,           -- Hash of content in content-addressed storage
    post_id INTEGER NOT NULL,           -- Foreign key to posts table
    revision INTEGER NOT NULL,          -- Revision number within post
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    pod TEXT NOT NULL,                  -- MainPod JSON proving document authenticity
    timestamp_pod TEXT NOT NULL,        -- Server SignedPod with timestamp
    uploader_id TEXT NOT NULL,          -- Username of uploader
    upvote_count_pod TEXT,              -- MainPod JSON proving upvote count
    tags TEXT DEFAULT '[]',             -- JSON array of tags
    authors TEXT DEFAULT '[]',          -- JSON array of author names
    reply_to INTEGER,                   -- Foreign key to documents table (optional)
    requested_post_id INTEGER,          -- Original post_id from request used in MainPod proof
    FOREIGN KEY (post_id) REFERENCES posts (id),
    FOREIGN KEY (reply_to) REFERENCES documents (id),
    UNIQUE (post_id, revision)
);
```

### `identity_servers`
```sql
CREATE TABLE identity_servers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    server_id TEXT NOT NULL UNIQUE,    -- Identity server identifier
    public_key TEXT NOT NULL,          -- Server's public key (JSON)
    challenge_pod TEXT NOT NULL,       -- Server's challenge SignedPod (JSON)
    identity_pod TEXT NOT NULL,        -- Identity server's response SignedPod (JSON)
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

### `upvotes`
```sql
CREATE TABLE upvotes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    document_id INTEGER NOT NULL,      -- Foreign key to documents table
    username TEXT NOT NULL,            -- Username of upvoter
    pod_json TEXT NOT NULL,            -- MainPod JSON proving upvote authenticity
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (document_id) REFERENCES documents (id),
    UNIQUE (document_id, username)     -- One upvote per user per document
);
```
