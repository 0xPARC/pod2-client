-- Drop drafts feature completely
DROP INDEX IF EXISTS idx_drafts_updated_at;
DROP TABLE IF EXISTS drafts;