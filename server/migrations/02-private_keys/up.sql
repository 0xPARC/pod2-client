CREATE TABLE private_keys (
    private_key TEXT PRIMARY KEY,
    key_type TEXT NOT NULL, /* e.g. Mock, Plonky2 */
    public_key TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);