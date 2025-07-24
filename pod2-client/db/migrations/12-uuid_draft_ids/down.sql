-- Revert draft IDs from UUID back to INTEGER (this will lose existing UUIDs)

-- Create new drafts table with INTEGER IDs
CREATE TABLE drafts_new (
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

-- Copy existing data with new auto-incrementing IDs
-- Note: This will lose the UUID identifiers and assign new integer IDs
INSERT INTO drafts_new (title, content_type, message, file_name, file_content, file_mime_type, url, tags, authors, reply_to, created_at, updated_at)
SELECT title, content_type, message, file_name, file_content, file_mime_type, url, tags, authors, reply_to, created_at, updated_at
FROM drafts;

-- Drop old table
DROP TABLE drafts;

-- Rename new table
ALTER TABLE drafts_new RENAME TO drafts;