-- Add session_id field to drafts table to prevent duplicate creation
ALTER TABLE drafts ADD COLUMN session_id TEXT;

-- Create index on session_id for efficient lookups
CREATE INDEX idx_drafts_session_id ON drafts(session_id);