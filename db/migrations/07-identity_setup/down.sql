-- Drop the index
DROP INDEX IF EXISTS idx_pods_mandatory;

-- Drop the app_setup_state table
DROP TABLE IF EXISTS app_setup_state;

-- Remove is_mandatory column from pods table
ALTER TABLE pods DROP COLUMN is_mandatory;