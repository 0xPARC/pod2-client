-- Change draft IDs from INTEGER to UUID (TEXT)

-- Create new drafts table with UUID IDs
CREATE TABLE drafts_new (
    id TEXT PRIMARY KEY, -- UUID stored as TEXT
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

-- Copy existing data with new UUIDs (if any exists)
-- Note: We can't preserve the old integer IDs, so existing drafts will get new UUIDs
INSERT INTO drafts_new (id, title, content_type, message, file_name, file_content, file_mime_type, url, tags, authors, reply_to, created_at, updated_at)
SELECT 
    lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-' || '4' || substr(hex(randomblob(2)), 2) || '-' || 
          substr('89ab', abs(random()) % 4 + 1, 1) || substr(hex(randomblob(2)), 2) || '-' || hex(randomblob(6))) as id,
    title, content_type, message, file_name, file_content, file_mime_type, url, tags, authors, reply_to, created_at, updated_at
FROM drafts;

-- Drop old table
DROP TABLE drafts;

-- Rename new table
ALTER TABLE drafts_new RENAME TO drafts;