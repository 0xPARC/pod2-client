CREATE TABLE drafts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    content_type TEXT NOT NULL CHECK (content_type IN ('message', 'file', 'url')),
    message TEXT,
    file_name TEXT,
    file_content BLOB,
    file_mime_type TEXT,
    url TEXT,
    tags TEXT, -- JSON array of strings
    authors TEXT, -- JSON array of strings
    reply_to TEXT, -- reply context if any (format: "post_id:document_id")
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Index for efficient querying by updated_at (for ordering drafts by last modified)
CREATE INDEX idx_drafts_updated_at ON drafts(updated_at DESC);