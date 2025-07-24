-- Reverse the simplification changes
-- This is for rollback if needed

-- Drop the new indexes
DROP INDEX IF EXISTS idx_drafts_single_open;
DROP INDEX IF EXISTS idx_drafts_is_open;

-- Remove is_open column
ALTER TABLE drafts DROP COLUMN is_open;

-- Re-add session_id column for backward compatibility
ALTER TABLE drafts ADD COLUMN session_id TEXT;

-- Recreate session_id index
CREATE INDEX idx_drafts_session_id ON drafts(session_id);