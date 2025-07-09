-- Extend existing private_keys table for P2P messaging

-- Add alias column for user-friendly key names
ALTER TABLE private_keys ADD COLUMN alias TEXT;

-- Add is_default column to designate the default signing key
ALTER TABLE private_keys ADD COLUMN is_default BOOLEAN DEFAULT FALSE;

-- Ensure only one default private key
CREATE UNIQUE INDEX IF NOT EXISTS idx_private_keys_default ON private_keys(is_default) WHERE is_default = TRUE;