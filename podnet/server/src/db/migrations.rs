use lazy_static::lazy_static;
use podnet_models::ReplyReference;
use rusqlite::OptionalExtension;
use rusqlite_migration::{M, Migrations};

lazy_static! {
    pub static ref MIGRATIONS: Migrations<'static> = Migrations::new(vec![
        M::up(
            "CREATE TABLE IF NOT EXISTS posts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                last_edited_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS documents (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content_id TEXT NOT NULL,
                post_id INTEGER NOT NULL,
                revision INTEGER NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                pod TEXT NOT NULL,
                timestamp_pod TEXT NOT NULL,
                uploader_id TEXT NOT NULL,
                upvote_count_pod TEXT,
                tags TEXT DEFAULT '[]',
                authors TEXT DEFAULT '[]',
                reply_to INTEGER,
                FOREIGN KEY (post_id) REFERENCES posts (id),
                UNIQUE (post_id, revision)
            );
            CREATE TABLE IF NOT EXISTS identity_servers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                server_id TEXT NOT NULL UNIQUE,
                public_key TEXT NOT NULL,
                challenge_pod TEXT NOT NULL,
                identity_pod TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            CREATE TABLE IF NOT EXISTS upvotes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                document_id INTEGER NOT NULL,
                username TEXT NOT NULL,
                pod_json TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (document_id) REFERENCES documents (id),
                UNIQUE (document_id, username)
            );",
        ),
        M::up(
            "ALTER TABLE posts ADD COLUMN parent_post_id INTEGER REFERENCES posts(id);
             ALTER TABLE posts ADD COLUMN thread_root_post_id INTEGER REFERENCES posts(id);
             ALTER TABLE posts ADD COLUMN reply_to_document_id INTEGER REFERENCES documents(id);
             CREATE INDEX IF NOT EXISTS idx_posts_parent_post_id ON posts(parent_post_id);
             CREATE INDEX IF NOT EXISTS idx_posts_thread_root_post_id ON posts(thread_root_post_id);"
        ),
        M::up("ALTER TABLE documents ADD COLUMN requested_post_id INTEGER;"),
        M::up("ALTER TABLE documents ADD COLUMN title TEXT NOT NULL DEFAULT '';"),
        M::up(
            "ALTER TABLE documents ADD COLUMN thread_root_id INTEGER;
             CREATE INDEX IF NOT EXISTS idx_thread_root_id ON documents(thread_root_id);"
        ),
        M::up_with_hook("-- V6 migrate reply_to column to text", |tx| {
            // Check if the migration has already been applied by checking if reply_to contains JSON
            let migration_check: rusqlite::Result<String, _> = tx.query_row(
                "SELECT reply_to FROM documents WHERE reply_to IS NOT NULL LIMIT 1",
                [],
                |row| {
                    Ok(row
                        .get::<_, Option<String>>(0)
                        .unwrap_or_default()
                        .unwrap_or_default())
                },
            );

            // If we can get a value and it's a number (not JSON), we need to migrate
            if let Ok(value) = migration_check
                && !value.is_empty() && value.parse::<i64>().is_ok() {
                    // Create a new table with the correct schema
                    tx.execute_batch(
                        "CREATE TABLE IF NOT EXISTS documents_new (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                content_id TEXT NOT NULL,
                                post_id INTEGER NOT NULL,
                                revision INTEGER NOT NULL,
                                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                                pod TEXT NOT NULL,
                                timestamp_pod TEXT NOT NULL,
                                uploader_id TEXT NOT NULL,
                                upvote_count_pod TEXT,
                                tags TEXT DEFAULT '[]',
                                authors TEXT DEFAULT '[]',
                                reply_to TEXT,
                                requested_post_id INTEGER,
                                title TEXT NOT NULL,
                                FOREIGN KEY (post_id) REFERENCES posts (id),
                                UNIQUE (post_id, revision)
                            );

                        INSERT INTO documents_new
                             SELECT id, content_id, post_id, revision, created_at, pod, timestamp_pod, 
                                    uploader_id, upvote_count_pod, tags, authors,
                                    CASE 
                                        WHEN reply_to IS NULL THEN NULL 
                                        ELSE json_object('post_id', -1, 'document_id', reply_to)
                                    END as reply_to,
                                    requested_post_id, title
                             FROM documents;
                        
                        DROP TABLE documents;
                        ALTER TABLE documents_new RENAME TO documents;",
                    )?;
                }
            Ok(())
        }),
        M::up_with_hook("-- V7 migrate thread_root_id data", |tx| {
             // Check if migration is needed - if any document has null thread_root_id
            let needs_migration: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM documents WHERE thread_root_id IS NULL)",
                [],
                |row| row.get(0),
            )?;

            if !needs_migration {
                return Ok(());
            }

            // Get all documents without thread_root_id
            let mut stmt = tx.prepare(
                "SELECT id, reply_to FROM documents WHERE thread_root_id IS NULL ORDER BY id",
            )?;

            let documents: Vec<(i64, Option<String>)> = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<rusqlite::Result<Vec<_>, _>>()?;

            // Process each document
            for (doc_id, reply_to_json) in documents {
                let thread_root_id = if let Some(reply_json) = reply_to_json {
                    // This is a reply - find the thread root by traversing up the chain
                    if let Ok(reply_ref) = serde_json::from_str::<ReplyReference>(&reply_json) {
                        let mut current_id = reply_ref.document_id;
                        let mut visited = std::collections::HashSet::new();
                        loop {
                            if visited.contains(&current_id) { break; }
                            visited.insert(current_id);
                            let next_reply: Option<String> = tx.query_row(
                                "SELECT reply_to FROM documents WHERE id = ?1",
                                [current_id], |row| row.get(0)).optional()?.flatten();
                            if let Some(next_json) = next_reply {
                                if let Ok(next_ref) = serde_json::from_str::<ReplyReference>(&next_json) {
                                    current_id = next_ref.document_id;
                                } else { break; }
                            } else { break; }
                        }
                        current_id
                    } else {
                        doc_id
                    }
                } else {
                    doc_id
                };

                // Update the document with its thread_root_id
                tx.execute(
                    "UPDATE documents SET thread_root_id = ?1 WHERE id = ?2",
                    [thread_root_id, doc_id],
                )?;
            }

            Ok(())
        }),
    ]);
}
