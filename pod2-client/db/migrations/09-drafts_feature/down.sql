-- Drop the drafts table and its associated index
DROP INDEX IF EXISTS idx_drafts_updated_at;
DROP TABLE IF EXISTS drafts;