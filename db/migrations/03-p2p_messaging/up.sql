-- Add P2P messaging tables for chats and inbox

-- Table for storing chat conversations with peers
CREATE TABLE chats (
    id TEXT PRIMARY KEY,
    peer_node_id TEXT NOT NULL UNIQUE,
    peer_alias TEXT,
    last_activity DATETIME NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    status TEXT DEFAULT 'active' CHECK (status IN ('active', 'archived', 'blocked'))
);

-- Table for storing individual messages within chats
CREATE TABLE chat_messages (
    id TEXT PRIMARY KEY,
    chat_id TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    space_id TEXT NOT NULL,
    pod_id TEXT NOT NULL,
    message_text TEXT, -- Extracted from POD["message"] if exists
    timestamp DATETIME NOT NULL,
    direction TEXT NOT NULL CHECK (direction IN ('sent', 'received')),
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (space_id, pod_id) REFERENCES pods(space, id) ON DELETE CASCADE
);

-- Table for storing unaccepted inbox messages
CREATE TABLE inbox_messages (
    id TEXT PRIMARY KEY,
    from_node_id TEXT NOT NULL,
    from_alias TEXT, -- Self-declared name from sender
    space_id TEXT NOT NULL,
    pod_id TEXT NOT NULL,
    message_text TEXT,
    received_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    status TEXT DEFAULT 'pending' CHECK (status IN ('pending', 'accepted', 'declined')),
    FOREIGN KEY (space_id, pod_id) REFERENCES pods(space, id) ON DELETE CASCADE
);

-- Indexes for performance
CREATE INDEX idx_chats_last_activity ON chats(last_activity DESC);
CREATE INDEX idx_chat_messages_chat_timestamp ON chat_messages(chat_id, timestamp);
CREATE INDEX idx_inbox_status ON inbox_messages(status, received_at DESC);