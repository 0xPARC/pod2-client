-- Add is_mandatory column to pods table to prevent deletion of identity PODs
ALTER TABLE pods ADD COLUMN is_mandatory BOOLEAN DEFAULT FALSE;

-- Create app_setup_state table to track completion of mandatory setup
CREATE TABLE app_setup_state (
    id INTEGER PRIMARY KEY,
    setup_completed BOOLEAN DEFAULT FALSE,
    identity_server_url TEXT,
    identity_server_id TEXT,
    identity_server_public_key TEXT,
    username TEXT,
    identity_pod_id TEXT,
    completed_at DATETIME,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Insert initial setup state record
INSERT INTO app_setup_state (id, setup_completed) VALUES (1, FALSE);

-- Create index for efficient lookup of mandatory pods
CREATE INDEX idx_pods_mandatory ON pods(is_mandatory) WHERE is_mandatory = TRUE;