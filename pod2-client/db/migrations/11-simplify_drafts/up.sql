-- Simplify drafts to single-draft model
-- Remove session_id complexity and add is_open tracking

-- Drop the session_id index
DROP INDEX IF EXISTS idx_drafts_session_id;

-- Remove session_id column
ALTER TABLE drafts DROP COLUMN session_id;

-- Add is_open boolean field to track if draft is currently being edited
ALTER TABLE drafts ADD COLUMN is_open BOOLEAN DEFAULT FALSE;

-- Create index for efficient open draft queries
CREATE INDEX idx_drafts_is_open ON drafts(is_open);

-- Ensure only one draft can be open at a time by enforcing constraint
-- This creates a unique partial index that only applies to open drafts
CREATE UNIQUE INDEX idx_drafts_single_open ON drafts(is_open) WHERE is_open = TRUE;