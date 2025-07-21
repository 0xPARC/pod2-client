-- Remove extensions to private_keys table

-- Remove the unique index
DROP INDEX IF EXISTS idx_private_keys_default;

-- Note: SQLite doesn't support DROP COLUMN, so we can't easily remove the added columns
-- In a production system, you'd need to recreate the table without these columns