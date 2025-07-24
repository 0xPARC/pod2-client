-- Remove session_id field from drafts table
DROP INDEX IF EXISTS idx_drafts_session_id;

-- Note: SQLite doesn't support DROP COLUMN, so we would need to recreate the table
-- For now, we'll leave the column in place for compatibility