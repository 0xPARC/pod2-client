use anyhow::Result;
use chrono::{DateTime, Utc};
use pod2::backends::plonky2::primitives::ec::curve::Point as PublicKey;
use rusqlite::{Connection, params};

pub fn initialize_database(db_path: &str) -> Result<Connection> {
    tracing::info!("Initializing GitHub identity database at: {}", db_path);

    let conn = Connection::open(db_path)?;

    // Create the users table with GitHub-specific fields
    conn.execute(
        "CREATE TABLE IF NOT EXISTS users (
            public_key_json TEXT PRIMARY KEY,
            username TEXT NOT NULL,
            github_username TEXT NOT NULL,
            github_user_id INTEGER UNIQUE NOT NULL,
            github_public_keys TEXT NOT NULL,
            oauth_verified_at TEXT NOT NULL,
            issued_at TEXT NOT NULL
        )",
        [],
    )?;

    tracing::info!("✓ GitHub identity database initialized successfully");
    Ok(conn)
}

pub fn insert_user_mapping(
    conn: &Connection,
    public_key: &PublicKey,
    username: &str,
    github_username: &str,
    github_user_id: i64,
    github_public_keys: &[String],
    oauth_verified_at: DateTime<Utc>,
) -> Result<()> {
    let public_key_json = serde_json::to_string(public_key)?;
    let github_public_keys_json = serde_json::to_string(github_public_keys)?;
    let issued_at = Utc::now();

    conn.execute(
        "INSERT OR REPLACE INTO users (
            public_key_json, 
            username, 
            github_username, 
            github_user_id, 
            github_public_keys, 
            oauth_verified_at, 
            issued_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            public_key_json,
            username,
            github_username,
            github_user_id,
            github_public_keys_json,
            oauth_verified_at.to_rfc3339(),
            issued_at.to_rfc3339()
        ],
    )?;

    tracing::info!(
        "✓ Stored GitHub user mapping: {} ({}) -> {}",
        username,
        github_username,
        public_key_json
    );
    Ok(())
}

pub fn get_username_by_public_key(
    conn: &Connection,
    public_key: &PublicKey,
) -> Result<Option<String>> {
    let public_key_json = serde_json::to_string(public_key)?;

    let mut stmt = conn.prepare("SELECT username FROM users WHERE public_key_json = ?1")?;
    let mut rows = stmt.query(params![public_key_json])?;

    if let Some(row) = rows.next()? {
        let username: String = row.get(0)?;
        Ok(Some(username))
    } else {
        Ok(None)
    }
}

pub fn user_exists_by_github_id(conn: &Connection, github_user_id: i64) -> Result<bool> {
    let mut stmt = conn.prepare("SELECT 1 FROM users WHERE github_user_id = ?1")?;
    let mut rows = stmt.query(params![github_user_id])?;
    Ok(rows.next()?.is_some())
}

pub fn delete_user_by_github_id(conn: &Connection, github_user_id: i64) -> Result<()> {
    let deleted_rows = conn.execute(
        "DELETE FROM users WHERE github_user_id = ?1",
        params![github_user_id],
    )?;

    if deleted_rows > 0 {
        tracing::info!(
            "✓ Deleted existing GitHub user record (ID: {})",
            github_user_id
        );
    }

    Ok(())
}
