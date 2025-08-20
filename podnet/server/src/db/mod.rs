use std::{collections::HashSet, sync::Mutex};

use hex::{FromHex, ToHex};
use pod2::{frontend::MainPod, middleware::Hash};
use podnet_models::{
    Document, DocumentContent, DocumentListItem, DocumentMetadata, DocumentPods, DocumentReplyTree,
    IdentityServer, Post, RawDocument, ReplyReference, Upvote, lazy_pod::LazyDeser,
};
use rusqlite::{Connection, OptionalExtension, Result};

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub async fn new(db_path: &str) -> anyhow::Result<Self> {
        let db_path = db_path.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = Connection::open(&db_path)?;
            let db = Database {
                conn: Mutex::new(conn),
            };
            db.init_tables()?;
            Ok(db)
        })
        .await?
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        // Create posts table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS posts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                last_edited_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // Create documents table (revisions of posts)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS documents (
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
                thread_root_id INTEGER,
                FOREIGN KEY (post_id) REFERENCES posts (id),
                FOREIGN KEY (thread_root_id) REFERENCES documents (id),
                UNIQUE (post_id, revision)
            )",
            [],
        )?;

        // Add requested_post_id column to existing databases (migration)
        // This will fail silently if the column already exists
        let _ = conn.execute(
            "ALTER TABLE documents ADD COLUMN requested_post_id INTEGER",
            [],
        );

        // Add title column to existing databases (migration)
        // This will fail silently if the column already exists
        let _ = conn.execute(
            "ALTER TABLE documents ADD COLUMN title TEXT NOT NULL DEFAULT ''",
            [],
        );

        // Add thread_root_id column to existing databases (migration)
        // This will fail silently if the column already exists
        let _ = conn.execute(
            "ALTER TABLE documents ADD COLUMN thread_root_id INTEGER",
            [],
        );

        // Migrate reply_to column to TEXT for ReplyReference support
        self.migrate_reply_to_column(&conn)?;

        // Migrate existing documents to populate thread_root_id
        self.migrate_thread_root_id(&conn)?;

        // Create index for thread_root_id for performance
        let _ = conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_thread_root_id ON documents(thread_root_id)",
            [],
        );

        // Create identity_servers table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS identity_servers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                server_id TEXT NOT NULL UNIQUE,
                public_key TEXT NOT NULL,
                challenge_pod TEXT NOT NULL,
                identity_pod TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;

        // Create upvotes table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS upvotes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                document_id INTEGER NOT NULL,
                username TEXT NOT NULL,
                pod_json TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (document_id) REFERENCES documents (id),
                UNIQUE (document_id, username)
            )",
            [],
        )?;

        Ok(())
    }

    fn migrate_reply_to_column(&self, conn: &Connection) -> Result<()> {
        // Check if the migration has already been applied by checking if reply_to contains JSON
        let migration_check: Result<String, _> = conn.query_row(
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
            && !value.is_empty()
            && value.parse::<i64>().is_ok()
        {
            // Create a new table with the correct schema
            conn.execute(
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
                    )",
                [],
            )?;

            // Copy data, converting INTEGER reply_to to JSON format
            conn.execute(
                "INSERT INTO documents_new 
                     SELECT id, content_id, post_id, revision, created_at, pod, timestamp_pod, 
                            uploader_id, upvote_count_pod, tags, authors,
                            CASE 
                                WHEN reply_to IS NULL THEN NULL 
                                ELSE json_object('post_id', -1, 'document_id', reply_to)
                            END as reply_to,
                            requested_post_id, title
                     FROM documents",
                [],
            )?;

            // Drop old table and rename new one
            conn.execute("DROP TABLE documents", [])?;
            conn.execute("ALTER TABLE documents_new RENAME TO documents", [])?;
        }

        Ok(())
    }

    fn migrate_thread_root_id(&self, conn: &Connection) -> Result<()> {
        // Check if migration is needed - if any document has null thread_root_id
        let needs_migration: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM documents WHERE thread_root_id IS NULL)",
            [],
            |row| row.get(0),
        )?;

        if !needs_migration {
            return Ok(());
        }

        // Get all documents without thread_root_id
        let mut stmt = conn.prepare(
            "SELECT id, reply_to FROM documents WHERE thread_root_id IS NULL ORDER BY id",
        )?;

        let documents: Vec<(i64, Option<String>)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;

        // Process each document
        for (doc_id, reply_to_json) in documents {
            let thread_root_id = if let Some(reply_json) = reply_to_json {
                // This is a reply - find the thread root by traversing up the chain
                if let Ok(reply_ref) = serde_json::from_str::<ReplyReference>(&reply_json) {
                    self.find_thread_root_id(conn, reply_ref.document_id)?
                } else {
                    // Invalid reply_to, treat as root document
                    doc_id
                }
            } else {
                // This is a root document - thread_root_id is itself
                doc_id
            };

            // Update the document with its thread_root_id
            conn.execute(
                "UPDATE documents SET thread_root_id = ?1 WHERE id = ?2",
                [thread_root_id, doc_id],
            )?;
        }

        Ok(())
    }

    fn find_thread_root_id(&self, conn: &Connection, mut document_id: i64) -> Result<i64> {
        let mut visited = std::collections::HashSet::new();

        loop {
            // Prevent infinite loops
            if visited.contains(&document_id) {
                return Ok(document_id);
            }
            visited.insert(document_id);

            // Check if this document has a reply_to
            let reply_to_json: Option<String> = conn
                .query_row(
                    "SELECT reply_to FROM documents WHERE id = ?1",
                    [document_id],
                    |row| row.get(0),
                )
                .optional()?
                .flatten();

            if let Some(reply_json) = reply_to_json {
                if let Ok(reply_ref) = serde_json::from_str::<ReplyReference>(&reply_json) {
                    document_id = reply_ref.document_id;
                } else {
                    // Invalid reply_to, this is the root
                    break;
                }
            } else {
                // No reply_to, this is the root
                break;
            }
        }

        Ok(document_id)
    }

    // Post methods
    pub fn create_post(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute("INSERT INTO posts DEFAULT VALUES", [])?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_post(&self, id: i64) -> Result<Option<Post>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT id, created_at, last_edited_at FROM posts WHERE id = ?1")?;

        let post = stmt
            .query_row([id], |row| {
                Ok(Post {
                    id: Some(row.get(0)?),
                    created_at: Some(row.get(1)?),
                    last_edited_at: Some(row.get(2)?),
                })
            })
            .optional()?;

        Ok(post)
    }

    pub fn get_all_posts(&self) -> Result<Vec<Post>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, created_at, last_edited_at FROM posts ORDER BY last_edited_at DESC",
        )?;

        let posts = stmt
            .query_map([], |row| {
                Ok(Post {
                    id: Some(row.get(0)?),
                    created_at: Some(row.get(1)?),
                    last_edited_at: Some(row.get(2)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(posts)
    }

    pub fn get_most_recent_modification_time(&self) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row("SELECT MAX(last_edited_at) FROM posts", [], |row| {
            row.get::<_, Option<String>>(0)
        });

        match result {
            Ok(timestamp) => Ok(timestamp),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn update_post_last_edited(&self, post_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE posts SET last_edited_at = CURRENT_TIMESTAMP WHERE id = ?1",
            [post_id],
        )?;
        Ok(())
    }

    // Document methods
    #[allow(clippy::too_many_arguments)]
    pub fn create_document(
        &self,
        content_id: &Hash,
        post_id: i64,
        pod: &MainPod,
        uploader_id: &str,
        tags: &HashSet<String>,
        authors: &HashSet<String>,
        reply_to: Option<ReplyReference>,
        requested_post_id: Option<i64>,
        title: &str,
        storage: &crate::storage::ContentAddressedStorage,
    ) -> Result<Document> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // Get the next revision number for this post
        let next_revision: i64 = tx.query_row(
            "SELECT COALESCE(MAX(revision), 0) + 1 FROM documents WHERE post_id = ?1",
            [post_id],
            |row| row.get(0),
        )?;

        // Convert pod to JSON string for storage
        let pod_json = serde_json::to_string(pod)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        let content_id_string: String = content_id.encode_hex();

        // Serialize tags to JSON
        let tags_json = serde_json::to_string(tags)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        // Serialize authors to JSON
        let authors_json = serde_json::to_string(authors)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        // Serialize reply_to to JSON if present
        let reply_to_json = if let Some(ref reply_ref) = reply_to {
            Some(
                serde_json::to_string(reply_ref)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?,
            )
        } else {
            None
        };

        // Determine thread_root_id
        let thread_root_id = if let Some(ref reply_ref) = reply_to {
            // This is a reply - get the thread_root_id from the parent document
            tx.query_row(
                "SELECT thread_root_id FROM documents WHERE id = ?1",
                [reply_ref.document_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|_| {
                rusqlite::Error::InvalidColumnName("Parent document not found".to_string())
            })?
        } else {
            // This is a root document - we'll set thread_root_id to document_id after insert
            -1i64 // Placeholder - will be updated after insert
        };

        // Insert document with empty timestamp_pod and null upvote_count_pod initially
        tx.execute(
            "INSERT INTO documents (content_id, post_id, revision, pod, timestamp_pod, uploader_id, upvote_count_pod, tags, authors, reply_to, requested_post_id, title, thread_root_id) VALUES (?1, ?2, ?3, ?4, '', ?5, NULL, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                content_id_string,
                post_id,
                next_revision,
                pod_json,
                uploader_id,
                tags_json,
                authors_json,
                reply_to_json,
                requested_post_id,
                title,
                thread_root_id,
            ],
        )?;

        let document_id = tx.last_insert_rowid();

        // Create timestamp pod with document_id and post_id
        let timestamp_pod =
            crate::pod::create_timestamp_pod_for_main_pod(pod, post_id, document_id)
                .map_err(rusqlite::Error::ToSqlConversionFailure)?;

        let timestamp_pod_json = serde_json::to_string(&timestamp_pod)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        // Update document with timestamp pod
        tx.execute(
            "UPDATE documents SET timestamp_pod = ?1 WHERE id = ?2",
            [&timestamp_pod_json, &document_id.to_string()],
        )?;

        // If this is a root document, update thread_root_id to point to itself
        if thread_root_id == -1 {
            tx.execute(
                "UPDATE documents SET thread_root_id = ?1 WHERE id = ?1",
                [document_id],
            )?;
        }

        // Update the post's last_edited_at timestamp
        tx.execute(
            "UPDATE posts SET last_edited_at = CURRENT_TIMESTAMP WHERE id = ?1",
            [post_id],
        )?;

        tx.commit()?;

        // Retrieve content from storage
        let content = storage
            .retrieve_document_content(content_id)
            .map_err(|_| {
                rusqlite::Error::InvalidColumnType(
                    0,
                    "content".to_string(),
                    rusqlite::types::Type::Text,
                )
            })?
            .ok_or_else(|| {
                rusqlite::Error::InvalidColumnType(
                    0,
                    "content".to_string(),
                    rusqlite::types::Type::Text,
                )
            })?;

        // Get upvote count (will be 0 for new document)
        let upvote_count = 0;

        // Create the metadata (without PODs)
        let metadata = DocumentMetadata {
            id: Some(document_id),
            content_id: *content_id,
            post_id,
            revision: next_revision,
            created_at: None, // Will be filled by database
            uploader_id: uploader_id.to_string(),
            upvote_count,
            tags: tags.clone(),
            authors: authors.clone(),
            reply_to,
            requested_post_id,
            title: title.to_string(),
        };

        // Create the pods
        let pods = DocumentPods {
            document_id,
            pod: LazyDeser::from_value(pod.clone()).map_err(|_| {
                rusqlite::Error::InvalidColumnType(
                    0,
                    "pod".to_string(),
                    rusqlite::types::Type::Text,
                )
            })?,
            timestamp_pod: LazyDeser::from_value(timestamp_pod).map_err(|_| {
                rusqlite::Error::InvalidColumnType(
                    0,
                    "timestamp_pod".to_string(),
                    rusqlite::types::Type::Text,
                )
            })?,
            upvote_count_pod: LazyDeser::from_value(None::<MainPod>).map_err(|_| {
                rusqlite::Error::InvalidColumnType(
                    0,
                    "upvote_count_pod".to_string(),
                    rusqlite::types::Type::Text,
                )
            })?, // Will be set by background task
        };

        Ok(Document {
            metadata,
            pods,
            content,
        })
    }

    pub fn get_raw_document(&self, id: i64) -> Result<Option<RawDocument>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, content_id, post_id, revision, created_at, pod, timestamp_pod, uploader_id, upvote_count_pod, tags, authors, reply_to, requested_post_id, title FROM documents WHERE id = ?1"
        )?;

        let document = stmt
            .query_row([id], |row| {
                let tags_json: String = row.get(9)?;
                let tags: HashSet<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                let authors_json: String = row.get(10)?;
                let authors: HashSet<String> =
                    serde_json::from_str(&authors_json).unwrap_or_default();
                let reply_to_json: Option<String> = row.get(11)?;
                let reply_to: Option<ReplyReference> =
                    reply_to_json.and_then(|json| serde_json::from_str(&json).ok());
                Ok(RawDocument {
                    id: Some(row.get(0)?),
                    content_id: row.get(1)?,
                    post_id: row.get(2)?,
                    revision: row.get(3)?,
                    created_at: Some(row.get(4)?),
                    pod: row.get(5)?,
                    timestamp_pod: row.get(6)?,
                    uploader_id: row.get(7)?,
                    upvote_count_pod: row.get(8)?,
                    tags,
                    authors,
                    reply_to,
                    requested_post_id: row.get(12)?,
                    title: row.get(13)?,
                })
            })
            .optional()?;

        Ok(document)
    }

    pub fn get_documents_by_post_id(&self, post_id: i64) -> Result<Vec<RawDocument>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, content_id, post_id, revision, created_at, pod, timestamp_pod, uploader_id, upvote_count_pod, tags, authors, reply_to, requested_post_id, title
             FROM documents WHERE post_id = ?1 ORDER BY revision DESC",
        )?;

        let documents = stmt
            .query_map([post_id], |row| {
                let tags_json: String = row.get(9)?;
                let tags: HashSet<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                let authors_json: String = row.get(10)?;
                let authors: HashSet<String> =
                    serde_json::from_str(&authors_json).unwrap_or_default();
                let reply_to_json: Option<String> = row.get(11)?;
                let reply_to: Option<ReplyReference> =
                    reply_to_json.and_then(|json| serde_json::from_str(&json).ok());
                Ok(RawDocument {
                    id: Some(row.get(0)?),
                    content_id: row.get(1)?,
                    post_id: row.get(2)?,
                    revision: row.get(3)?,
                    created_at: Some(row.get(4)?),
                    pod: row.get(5)?,
                    timestamp_pod: row.get(6)?,
                    uploader_id: row.get(7)?,
                    upvote_count_pod: row.get(8)?,
                    tags,
                    authors,
                    reply_to,
                    requested_post_id: row.get(12)?,
                    title: row.get(13)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(documents)
    }

    pub fn get_latest_document_by_post_id(&self, post_id: i64) -> Result<Option<RawDocument>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, content_id, post_id, revision, created_at, pod, timestamp_pod, uploader_id, upvote_count_pod, tags, authors, reply_to, requested_post_id, title
             FROM documents WHERE post_id = ?1 ORDER BY revision DESC LIMIT 1",
        )?;

        let document = stmt
            .query_row([post_id], |row| {
                let tags_json: String = row.get(9)?;
                let tags: HashSet<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                let authors_json: String = row.get(10)?;
                let authors: HashSet<String> =
                    serde_json::from_str(&authors_json).unwrap_or_default();
                let reply_to_json: Option<String> = row.get(11)?;
                let reply_to: Option<ReplyReference> =
                    reply_to_json.and_then(|json| serde_json::from_str(&json).ok());
                Ok(RawDocument {
                    id: Some(row.get(0)?),
                    content_id: row.get(1)?,
                    post_id: row.get(2)?,
                    revision: row.get(3)?,
                    created_at: Some(row.get(4)?),
                    pod: row.get(5)?,
                    timestamp_pod: row.get(6)?,
                    uploader_id: row.get(7)?,
                    upvote_count_pod: row.get(8)?,
                    tags,
                    authors,
                    reply_to,
                    requested_post_id: row.get(12)?,
                    title: row.get(13)?,
                })
            })
            .optional()?;

        Ok(document)
    }

    pub fn get_all_documents(&self) -> Result<Vec<RawDocument>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, content_id, post_id, revision, created_at, pod, timestamp_pod, uploader_id, upvote_count_pod, tags, authors, reply_to, requested_post_id, title
             FROM documents ORDER BY created_at DESC",
        )?;

        let documents = stmt
            .query_map([], |row| {
                let tags_json: String = row.get(9)?;
                let tags: HashSet<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                let authors_json: String = row.get(10)?;
                let authors: HashSet<String> =
                    serde_json::from_str(&authors_json).unwrap_or_default();
                let reply_to_json: Option<String> = row.get(11)?;
                let reply_to: Option<ReplyReference> =
                    reply_to_json.and_then(|json| serde_json::from_str(&json).ok());
                Ok(RawDocument {
                    id: Some(row.get(0)?),
                    content_id: row.get(1)?,
                    post_id: row.get(2)?,
                    revision: row.get(3)?,
                    created_at: Some(row.get(4)?),
                    pod: row.get(5)?,
                    timestamp_pod: row.get(6)?,
                    uploader_id: row.get(7)?,
                    upvote_count_pod: row.get(8)?,
                    tags,
                    authors,
                    reply_to,
                    requested_post_id: row.get(12)?,
                    title: row.get(13)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(documents)
    }

    // Identity server methods
    pub fn create_identity_server(
        &self,
        server_id: &str,
        public_key: &str,
        challenge_pod: &str,
        identity_pod: &str,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO identity_servers (server_id, public_key, challenge_pod, identity_pod) VALUES (?1, ?2, ?3, ?4)",
            [server_id, public_key, challenge_pod, identity_pod],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_identity_server_by_id(&self, server_id: &str) -> Result<Option<IdentityServer>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, server_id, public_key, challenge_pod, identity_pod, created_at FROM identity_servers WHERE server_id = ?1",
        )?;

        let identity_server = stmt
            .query_row([server_id], |row| {
                Ok(IdentityServer {
                    id: Some(row.get(0)?),
                    server_id: row.get(1)?,
                    public_key: row.get(2)?,
                    challenge_pod: row.get(3)?,
                    identity_pod: row.get(4)?,
                    created_at: Some(row.get(5)?),
                })
            })
            .optional()?;

        Ok(identity_server)
    }

    pub fn get_identity_server_by_public_key(
        &self,
        public_key: &str,
    ) -> Result<Option<IdentityServer>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, server_id, public_key, challenge_pod, identity_pod, created_at FROM identity_servers WHERE public_key = ?1",
        )?;

        let identity_server = stmt
            .query_row([public_key], |row| {
                Ok(IdentityServer {
                    id: Some(row.get(0)?),
                    server_id: row.get(1)?,
                    public_key: row.get(2)?,
                    challenge_pod: row.get(3)?,
                    identity_pod: row.get(4)?,
                    created_at: Some(row.get(5)?),
                })
            })
            .optional()?;

        Ok(identity_server)
    }

    pub fn get_all_identity_servers(&self) -> Result<Vec<IdentityServer>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, server_id, public_key, challenge_pod, identity_pod, created_at FROM identity_servers ORDER BY created_at DESC",
        )?;

        let identity_servers = stmt
            .query_map([], |row| {
                Ok(IdentityServer {
                    id: Some(row.get(0)?),
                    server_id: row.get(1)?,
                    public_key: row.get(2)?,
                    challenge_pod: row.get(3)?,
                    identity_pod: row.get(4)?,
                    created_at: Some(row.get(5)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(identity_servers)
    }

    // Upvote methods
    pub fn create_upvote(&self, document_id: i64, username: &str, pod_json: &str) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO upvotes (document_id, username, pod_json) VALUES (?1, ?2, ?3)",
            [&document_id.to_string(), username, pod_json],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_upvote_count(&self, document_id: i64) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let count = conn.query_row(
            "SELECT COUNT(*) FROM upvotes WHERE document_id = ?1",
            [document_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn get_upvotes_by_document_id(&self, document_id: i64) -> Result<Vec<Upvote>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, document_id, username, pod_json, created_at FROM upvotes WHERE document_id = ?1",
        )?;

        let upvotes = stmt
            .query_map([document_id], |row| {
                Ok(Upvote {
                    id: Some(row.get(0)?),
                    document_id: row.get(1)?,
                    username: row.get(2)?,
                    pod_json: row.get(3)?,
                    created_at: Some(row.get(4)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(upvotes)
    }

    // Helper method to convert RawDocument to DocumentMetadata (without PODs)
    pub fn raw_document_to_metadata(&self, raw_doc: RawDocument) -> Result<DocumentMetadata> {
        // Get upvote count
        let upvote_count = raw_doc
            .id
            .map(|id| self.get_upvote_count(id).unwrap_or(0))
            .unwrap_or(0);

        let content_id = Hash::from_hex(raw_doc.content_id).map_err(|_| {
            rusqlite::Error::InvalidColumnType(
                0,
                "content".to_string(),
                rusqlite::types::Type::Text,
            )
        })?;

        Ok(DocumentMetadata {
            id: raw_doc.id,
            content_id,
            post_id: raw_doc.post_id,
            revision: raw_doc.revision,
            created_at: raw_doc.created_at,
            uploader_id: raw_doc.uploader_id,
            upvote_count,
            tags: raw_doc.tags,
            authors: raw_doc.authors,
            reply_to: raw_doc.reply_to,
            requested_post_id: raw_doc.requested_post_id,
            title: raw_doc.title,
        })
    }

    // Helper method to convert RawDocument to DocumentPods
    pub fn raw_document_to_pods(&self, raw_doc: RawDocument) -> Result<DocumentPods> {
        let document_id = raw_doc.id.ok_or_else(|| {
            rusqlite::Error::InvalidColumnType(0, "id".to_string(), rusqlite::types::Type::Integer)
        })?;

        // Create lazy pod wrappers instead of deserializing immediately
        let pod = LazyDeser::from_json_string(raw_doc.pod).map_err(|_| {
            rusqlite::Error::InvalidColumnType(0, "pod".to_string(), rusqlite::types::Type::Text)
        })?;
        let timestamp_pod = LazyDeser::from_json_string(raw_doc.timestamp_pod).map_err(|_| {
            rusqlite::Error::InvalidColumnType(
                0,
                "timestamp_pod".to_string(),
                rusqlite::types::Type::Text,
            )
        })?;

        // For optional MainPod, we need to create the JSON for Option<MainPod>
        let upvote_count_pod_json =
            serde_json::to_string(&raw_doc.upvote_count_pod).map_err(|_| {
                rusqlite::Error::InvalidColumnType(
                    0,
                    "upvote_count_pod".to_string(),
                    rusqlite::types::Type::Text,
                )
            })?;
        let upvote_count_pod =
            LazyDeser::from_json_string(upvote_count_pod_json).map_err(|_| {
                rusqlite::Error::InvalidColumnType(
                    0,
                    "upvote_count_pod".to_string(),
                    rusqlite::types::Type::Text,
                )
            })?;

        Ok(DocumentPods {
            document_id,
            pod,
            timestamp_pod,
            upvote_count_pod,
        })
    }

    // Get document metadata only (no content)
    pub fn get_document_metadata(&self, id: i64) -> Result<Option<DocumentMetadata>> {
        match self.get_raw_document(id)? {
            Some(raw_doc) => Ok(Some(self.raw_document_to_metadata(raw_doc)?)),
            None => Ok(None),
        }
    }

    // Get document with content from storage
    pub fn get_document(
        &self,
        id: i64,
        storage: &crate::storage::ContentAddressedStorage,
    ) -> Result<Option<Document>> {
        match self.get_raw_document(id)? {
            Some(raw_doc) => {
                let metadata = self.raw_document_to_metadata(raw_doc.clone())?;
                let pods = self.raw_document_to_pods(raw_doc.clone())?;
                let content_hash = Hash::from_hex(raw_doc.content_id).map_err(|_| {
                    rusqlite::Error::InvalidColumnType(
                        0,
                        "content_id".to_string(),
                        rusqlite::types::Type::Text,
                    )
                })?;

                // Retrieve content from storage
                let content = storage
                    .retrieve_document_content(&content_hash)
                    .map_err(|_| {
                        rusqlite::Error::InvalidColumnType(
                            0,
                            "content".to_string(),
                            rusqlite::types::Type::Text,
                        )
                    })?
                    .ok_or_else(|| {
                        rusqlite::Error::InvalidColumnType(
                            0,
                            "content".to_string(),
                            rusqlite::types::Type::Text,
                        )
                    })?;

                Ok(Some(Document {
                    metadata,
                    pods,
                    content,
                }))
            }
            None => Ok(None),
        }
    }

    // Get all documents metadata only
    pub fn get_all_documents_metadata(&self) -> Result<Vec<DocumentMetadata>> {
        let raw_documents = self.get_all_documents()?;
        let mut documents_metadata = Vec::new();

        for raw_doc in raw_documents {
            documents_metadata.push(self.raw_document_to_metadata(raw_doc)?);
        }

        Ok(documents_metadata)
    }

    // Get top-level documents with latest reply information for list views
    pub fn get_top_level_documents_with_latest_reply(&self) -> Result<Vec<DocumentListItem>> {
        // First, query all raw rows while holding the lock
        let rows: Vec<(RawDocument, Option<String>, Option<String>)> = {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT 
                    d.id, d.content_id, d.post_id, d.revision, d.created_at, d.pod, d.timestamp_pod,
                    d.uploader_id, d.upvote_count_pod, d.tags, d.authors, d.reply_to, d.requested_post_id, d.title,
                    (
                        SELECT r.created_at FROM documents r
                        WHERE r.thread_root_id = d.id AND r.reply_to IS NOT NULL
                        ORDER BY r.created_at DESC
                        LIMIT 1
                    ) AS latest_reply_at,
                    (
                        SELECT r.uploader_id FROM documents r
                        WHERE r.thread_root_id = d.id AND r.reply_to IS NOT NULL
                        ORDER BY r.created_at DESC
                        LIMIT 1
                    ) AS latest_reply_by
                 FROM documents d
                 WHERE d.thread_root_id = d.id
                 ORDER BY d.created_at DESC",
            )?;

            stmt.query_map([], |row| {
                // Parse the same fields as RawDocument
                let tags_json: String = row.get(9)?;
                let tags: HashSet<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                let authors_json: String = row.get(10)?;
                let authors: HashSet<String> =
                    serde_json::from_str(&authors_json).unwrap_or_default();
                let reply_to_json: Option<String> = row.get(11)?;
                let reply_to: Option<ReplyReference> =
                    reply_to_json.and_then(|json| serde_json::from_str(&json).ok());

                // Create RawDocument first
                let raw_doc = RawDocument {
                    id: Some(row.get(0)?),
                    content_id: row.get(1)?,
                    post_id: row.get(2)?,
                    revision: row.get(3)?,
                    created_at: Some(row.get(4)?),
                    pod: row.get(5)?,
                    timestamp_pod: row.get(6)?,
                    uploader_id: row.get(7)?,
                    upvote_count_pod: row.get(8)?,
                    tags,
                    authors,
                    reply_to,
                    requested_post_id: row.get(12)?,
                    title: row.get(13)?,
                };

                let latest_reply_at: Option<String> = row.get(14)?;
                let latest_reply_by: Option<String> = row.get(15)?;

                Ok((raw_doc, latest_reply_at, latest_reply_by))
            })?
            .collect::<Result<Vec<_>, _>>()?
        };

        // Now, outside of the DB lock, convert to DocumentListItem
        let mut result = Vec::new();
        for (raw_doc, latest_reply_at, latest_reply_by) in rows {
            let metadata = self.raw_document_to_metadata(raw_doc)?;
            result.push(DocumentListItem {
                metadata,
                latest_reply_at,
                latest_reply_by,
            });
        }

        Ok(result)
    }

    // Get documents by post ID (metadata only)
    pub fn get_documents_metadata_by_post_id(&self, post_id: i64) -> Result<Vec<DocumentMetadata>> {
        let raw_documents = self.get_documents_by_post_id(post_id)?;
        let mut documents_metadata = Vec::new();

        for raw_doc in raw_documents {
            documents_metadata.push(self.raw_document_to_metadata(raw_doc)?);
        }

        Ok(documents_metadata)
    }

    pub fn user_has_upvoted(&self, document_id: i64, username: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM upvotes WHERE document_id = ?1 AND username = ?2",
            [&document_id.to_string(), username],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Delete a document and return the uploader username for verification
    pub fn delete_document(&self, document_id: i64) -> Result<String> {
        let conn = self.conn.lock().unwrap();

        // First get the document to verify it exists and get uploader info
        let uploader_id: String = conn.query_row(
            "SELECT uploader_id FROM documents WHERE id = ?1",
            [&document_id.to_string()],
            |row| row.get(0),
        )?;

        // Delete the document
        let deleted_rows = conn.execute(
            "DELETE FROM documents WHERE id = ?1",
            [&document_id.to_string()],
        )?;

        if deleted_rows == 0 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }

        // Also delete associated upvotes
        conn.execute(
            "DELETE FROM upvotes WHERE document_id = ?1",
            [&document_id.to_string()],
        )?;

        tracing::info!("Deleted document {document_id} and associated upvotes");
        Ok(uploader_id)
    }

    /// Get uploader username for a document
    pub fn get_document_uploader(&self, document_id: i64) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT uploader_id FROM documents WHERE id = ?1",
            [&document_id.to_string()],
            |row| row.get::<_, String>(0),
        );

        match result {
            Ok(uploader_id) => Ok(Some(uploader_id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn update_upvote_count_pod(&self, document_id: i64, upvote_count_pod: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE documents SET upvote_count_pod = ?1 WHERE id = ?2",
            [upvote_count_pod, &document_id.to_string()],
        )?;
        Ok(())
    }

    pub fn get_upvote_count_pod(&self, document_id: i64) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT upvote_count_pod FROM documents WHERE id = ?1",
            [document_id],
            |row| row.get::<_, Option<String>>(0),
        );

        match result {
            Ok(pod) => Ok(pod),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    // Get documents that reply to a specific document
    pub fn get_replies_to_document(&self, document_id: i64) -> Result<Vec<RawDocument>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, content_id, post_id, revision, created_at, pod, timestamp_pod, uploader_id, upvote_count_pod, tags, authors, reply_to, requested_post_id, title
             FROM documents WHERE json_extract(reply_to, '$.document_id') = ?1 ORDER BY created_at ASC",
        )?;

        let documents = stmt
            .query_map([document_id], |row| {
                let tags_json: String = row.get(9)?;
                let tags: HashSet<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                let authors_json: String = row.get(10)?;
                let authors: HashSet<String> =
                    serde_json::from_str(&authors_json).unwrap_or_default();
                let reply_to_json: Option<String> = row.get(11)?;
                let reply_to: Option<ReplyReference> =
                    reply_to_json.and_then(|json| serde_json::from_str(&json).ok());
                Ok(RawDocument {
                    id: Some(row.get(0)?),
                    content_id: row.get(1)?,
                    post_id: row.get(2)?,
                    revision: row.get(3)?,
                    created_at: Some(row.get(4)?),
                    pod: row.get(5)?,
                    timestamp_pod: row.get(6)?,
                    uploader_id: row.get(7)?,
                    upvote_count_pod: row.get(8)?,
                    tags,
                    authors,
                    reply_to,
                    requested_post_id: row.get(12)?,
                    title: row.get(13)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(documents)
    }

    // Get complete reply tree for a specific document using thread_root_id
    pub fn get_reply_tree_for_document(
        &self,
        document_id: i64,
        storage: &crate::storage::ContentAddressedStorage,
    ) -> Result<Option<DocumentReplyTree>> {
        // First verify the document exists and get its thread_root_id
        let document_thread_root_id = match self.get_document_thread_root_id(document_id)? {
            Some(thread_root_id) => thread_root_id,
            None => return Ok(None), // Document doesn't exist
        };

        // Get all documents in the same thread using simple query
        let all_thread_documents = self.get_documents_by_thread_root_id(document_thread_root_id)?;

        // Build the tree structure from the flat list, starting with the requested document
        self.build_reply_tree_from_documents(all_thread_documents, document_id, storage)
    }

    // Helper method to get thread_root_id for a document
    pub fn get_document_thread_root_id(&self, document_id: i64) -> Result<Option<i64>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT thread_root_id FROM documents WHERE id = ?1",
            [document_id],
            |row| row.get(0),
        )
        .optional()
    }

    // Helper method to get all documents in a thread
    pub fn get_documents_by_thread_root_id(&self, thread_root_id: i64) -> Result<Vec<RawDocument>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, content_id, post_id, revision, created_at, pod, timestamp_pod, uploader_id, upvote_count_pod, tags, authors, reply_to, requested_post_id, title, thread_root_id
             FROM documents WHERE thread_root_id = ?1 ORDER BY created_at ASC",
        )?;

        let documents = stmt
            .query_map([thread_root_id], |row| {
                let tags_json: String = row.get(9)?;
                let tags: HashSet<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                let authors_json: String = row.get(10)?;
                let authors: HashSet<String> =
                    serde_json::from_str(&authors_json).unwrap_or_default();
                let reply_to_json: Option<String> = row.get(11)?;
                let reply_to: Option<ReplyReference> =
                    reply_to_json.and_then(|json| serde_json::from_str(&json).ok());

                Ok(RawDocument {
                    id: Some(row.get(0)?),
                    content_id: row.get(1)?,
                    post_id: row.get(2)?,
                    revision: row.get(3)?,
                    created_at: Some(row.get(4)?),
                    pod: row.get(5)?,
                    timestamp_pod: row.get(6)?,
                    uploader_id: row.get(7)?,
                    upvote_count_pod: row.get(8)?,
                    tags,
                    authors,
                    reply_to,
                    requested_post_id: row.get(12)?,
                    title: row.get(13)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(documents)
    }

    // Helper method to build tree structure from flat list of documents
    fn build_reply_tree_from_documents(
        &self,
        raw_documents: Vec<RawDocument>,
        requested_document_id: i64,
        storage: &crate::storage::ContentAddressedStorage,
    ) -> Result<Option<DocumentReplyTree>> {
        use std::collections::HashMap;

        if raw_documents.is_empty() {
            return Ok(None);
        }

        // Convert raw documents to metadata and content, organize by ID
        let mut document_map: HashMap<i64, DocumentMetadata> = HashMap::new();
        let mut content_map: HashMap<i64, DocumentContent> = HashMap::new();
        let mut children_map: HashMap<i64, Vec<i64>> = HashMap::new();

        for raw_doc in raw_documents {
            let doc_id = raw_doc.id.unwrap_or(-1);
            let metadata = self.raw_document_to_metadata(raw_doc.clone())?;

            // Fetch content from storage
            let content_hash = Hash::from_hex(raw_doc.content_id).map_err(|_| {
                rusqlite::Error::InvalidColumnType(
                    0,
                    "content_id".to_string(),
                    rusqlite::types::Type::Text,
                )
            })?;
            let content = storage
                .retrieve_document_content(&content_hash)
                .map_err(|_| rusqlite::Error::InvalidPath("storage error".into()))?
                .ok_or_else(|| {
                    rusqlite::Error::InvalidPath("content not found in storage".into())
                })?;

            document_map.insert(doc_id, metadata.clone());
            content_map.insert(doc_id, content);

            // Build parent-child relationships
            if let Some(ref reply_to) = metadata.reply_to {
                children_map
                    .entry(reply_to.document_id)
                    .or_default()
                    .push(doc_id);
            }
        }

        // Find the requested document to use as root
        if !document_map.contains_key(&requested_document_id) {
            return Ok(None);
        }

        // Recursively build the tree starting from the requested document
        fn build_tree_node(
            document_id: i64,
            document_map: &HashMap<i64, DocumentMetadata>,
            content_map: &HashMap<i64, DocumentContent>,
            children_map: &HashMap<i64, Vec<i64>>,
        ) -> Option<DocumentReplyTree> {
            let document = document_map.get(&document_id)?.clone();
            let content = content_map.get(&document_id)?.clone();
            let child_ids = children_map.get(&document_id).cloned().unwrap_or_default();

            let mut replies = Vec::new();
            for child_id in child_ids {
                if let Some(child_tree) =
                    build_tree_node(child_id, document_map, content_map, children_map)
                {
                    replies.push(child_tree);
                }
            }

            // Sort replies by creation time
            replies.sort_by(|a, b| {
                a.document
                    .created_at
                    .as_ref()
                    .cmp(&b.document.created_at.as_ref())
            });

            Some(DocumentReplyTree {
                document,
                content,
                replies,
            })
        }

        Ok(build_tree_node(
            requested_document_id,
            &document_map,
            &content_map,
            &children_map,
        ))
    }
}

#[cfg(test)]
pub mod tests {
    use std::collections::HashSet;

    use podnet_models::DocumentContent;

    use super::*;

    // Test helper functions
    fn create_test_database() -> Database {
        // Use in-memory database for testing
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(Database::new(":memory:"))
            .expect("Failed to create test database")
    }

    fn create_test_storage() -> crate::storage::ContentAddressedStorage {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("podnet_test_storage_{timestamp}"));
        crate::storage::ContentAddressedStorage::new(temp_dir.to_str().unwrap())
            .expect("Failed to create test storage")
    }

    pub fn insert_dummy_document(
        db: &Database,
        storage: &crate::storage::ContentAddressedStorage,
        title: &str,
        reply_to: Option<ReplyReference>,
    ) -> i64 {
        let conn = db.conn.lock().unwrap();

        // Create dummy content and store it
        let content = DocumentContent {
            message: Some(format!("Test content for {title}")),
            file: None,
            url: None,
        };
        let content_hash = storage
            .store_document_content(&content)
            .expect("Failed to store test content")
            .encode_hex::<String>();

        // Create dummy data
        let dummy_pod_json = r#"{"mock": "pod"}"#;
        let dummy_timestamp_pod_json = r#"{"mock": "timestamp_pod"}"#;
        let tags_json = "[]";
        let authors_json = "[]";
        let reply_to_json = reply_to.as_ref().map(|r| serde_json::to_string(r).unwrap());

        // Determine thread_root_id
        let thread_root_id = if let Some(ref reply_ref) = reply_to {
            // This is a reply - get thread_root_id from parent
            conn.query_row(
                "SELECT thread_root_id FROM documents WHERE id = ?1",
                [reply_ref.document_id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(reply_ref.document_id) // Fallback to parent ID if no thread_root_id
        } else {
            // This will be a root document - use placeholder, update after insert
            -1i64
        };

        // First ensure we have a post
        conn.execute("INSERT OR IGNORE INTO posts (id) VALUES (1)", [])
            .unwrap();

        if thread_root_id == -1 {
            // Root document: insert without thread_root_id first, then update
            let _result = conn.execute(
                "INSERT INTO documents (content_id, post_id, revision, pod, timestamp_pod, uploader_id, upvote_count_pod, tags, authors, reply_to, requested_post_id, title) 
                 VALUES (?1, 1, (SELECT COALESCE(MAX(revision), 0) + 1 FROM documents WHERE post_id = 1), ?2, ?3, 'test_user', NULL, ?4, ?5, ?6, NULL, ?7)",
                (
                    &content_hash,
                    dummy_pod_json,
                    dummy_timestamp_pod_json,
                    tags_json,
                    authors_json,
                    reply_to_json.as_deref(),
                    title,
                ),
            ).unwrap();

            let document_id = conn.last_insert_rowid();

            // Update thread_root_id to point to itself
            conn.execute(
                "UPDATE documents SET thread_root_id = ?1 WHERE id = ?1",
                [document_id],
            )
            .unwrap();

            document_id
        } else {
            // Reply document: insert with proper thread_root_id
            let _result = conn.execute(
                "INSERT INTO documents (content_id, post_id, revision, pod, timestamp_pod, uploader_id, upvote_count_pod, tags, authors, reply_to, requested_post_id, title, thread_root_id) 
                 VALUES (?1, 1, (SELECT COALESCE(MAX(revision), 0) + 1 FROM documents WHERE post_id = 1), ?2, ?3, 'test_user', NULL, ?4, ?5, ?6, NULL, ?7, ?8)",
                (
                    &content_hash,
                    dummy_pod_json,
                    dummy_timestamp_pod_json,
                    tags_json,
                    authors_json,
                    reply_to_json.as_deref(),
                    title,
                    thread_root_id,
                ),
            ).unwrap();

            conn.last_insert_rowid()
        }
    }

    pub fn create_reply_reference(document_id: i64) -> ReplyReference {
        ReplyReference {
            post_id: 1,
            document_id,
        }
    }

    #[test]
    fn test_single_document_no_replies() {
        let db = create_test_database();
        let storage = create_test_storage();
        let doc_id = insert_dummy_document(&db, &storage, "Root Document", None);

        let tree = db.get_reply_tree_for_document(doc_id, &storage).unwrap();
        assert!(tree.is_some());

        let tree = tree.unwrap();
        assert_eq!(tree.document.title, "Root Document");
        assert_eq!(
            tree.content.message,
            Some("Test content for Root Document".to_string())
        );
        assert_eq!(tree.replies.len(), 0);
    }

    #[test]
    fn test_linear_reply_chain() {
        let db = create_test_database();
        let storage = create_test_storage();

        // Create: A -> B -> C -> D
        let doc_a = insert_dummy_document(&db, &storage, "Doc A", None);
        let doc_b =
            insert_dummy_document(&db, &storage, "Doc B", Some(create_reply_reference(doc_a)));
        let doc_c =
            insert_dummy_document(&db, &storage, "Doc C", Some(create_reply_reference(doc_b)));
        let _doc_d =
            insert_dummy_document(&db, &storage, "Doc D", Some(create_reply_reference(doc_c)));

        let tree = db
            .get_reply_tree_for_document(doc_a, &storage)
            .unwrap()
            .unwrap();

        // Verify structure: A has 1 reply (B)
        assert_eq!(tree.document.title, "Doc A");
        assert_eq!(tree.replies.len(), 1);

        // B has 1 reply (C)
        let reply_b = &tree.replies[0];
        assert_eq!(reply_b.document.title, "Doc B");
        assert_eq!(reply_b.replies.len(), 1);

        // C has 1 reply (D)
        let reply_c = &reply_b.replies[0];
        assert_eq!(reply_c.document.title, "Doc C");
        assert_eq!(reply_c.replies.len(), 1);

        // D has no replies
        let reply_d = &reply_c.replies[0];
        assert_eq!(reply_d.document.title, "Doc D");
        assert_eq!(reply_d.replies.len(), 0);
    }

    #[test]
    fn test_branching_reply_tree() {
        let db = create_test_database();
        let storage = create_test_storage();

        // Create: A -> B, C; B -> D, E; C -> F
        let doc_a = insert_dummy_document(&db, &storage, "Doc A", None);
        let doc_b =
            insert_dummy_document(&db, &storage, "Doc B", Some(create_reply_reference(doc_a)));
        let doc_c =
            insert_dummy_document(&db, &storage, "Doc C", Some(create_reply_reference(doc_a)));
        let _doc_d =
            insert_dummy_document(&db, &storage, "Doc D", Some(create_reply_reference(doc_b)));
        let _doc_e =
            insert_dummy_document(&db, &storage, "Doc E", Some(create_reply_reference(doc_b)));
        let _doc_f =
            insert_dummy_document(&db, &storage, "Doc F", Some(create_reply_reference(doc_c)));

        let tree = db
            .get_reply_tree_for_document(doc_a, &storage)
            .unwrap()
            .unwrap();

        // A has 2 direct replies (B and C)
        assert_eq!(tree.document.title, "Doc A");
        assert_eq!(tree.replies.len(), 2);

        // Find B and C replies (order might vary)
        let mut reply_b = None;
        let mut reply_c = None;
        for reply in &tree.replies {
            match reply.document.title.as_str() {
                "Doc B" => reply_b = Some(reply),
                "Doc C" => reply_c = Some(reply),
                _ => panic!("Unexpected reply title: {}", reply.document.title),
            }
        }

        let reply_b = reply_b.unwrap();
        let reply_c = reply_c.unwrap();

        // B has 2 replies (D and E)
        assert_eq!(reply_b.replies.len(), 2);
        let b_reply_titles: HashSet<_> = reply_b
            .replies
            .iter()
            .map(|r| r.document.title.as_str())
            .collect();
        assert!(b_reply_titles.contains("Doc D"));
        assert!(b_reply_titles.contains("Doc E"));

        // C has 1 reply (F)
        assert_eq!(reply_c.replies.len(), 1);
        assert_eq!(reply_c.replies[0].document.title, "Doc F");
    }

    #[test]
    fn test_nonexistent_document() {
        let db = create_test_database();
        let storage = create_test_storage();
        let tree = db.get_reply_tree_for_document(99999, &storage).unwrap();
        assert!(tree.is_none());
    }

    #[test]
    fn test_document_with_empty_reply_to() {
        let db = create_test_database();

        // Create two separate root documents
        let storage = create_test_storage();
        let doc1 = insert_dummy_document(&db, &storage, "Root 1", None);
        let doc2 = insert_dummy_document(&db, &storage, "Root 2", None);

        let tree1 = db
            .get_reply_tree_for_document(doc1, &storage)
            .unwrap()
            .unwrap();
        assert_eq!(tree1.document.title, "Root 1");
        assert_eq!(tree1.replies.len(), 0);

        let tree2 = db
            .get_reply_tree_for_document(doc2, &storage)
            .unwrap()
            .unwrap();
        assert_eq!(tree2.document.title, "Root 2");
        assert_eq!(tree2.replies.len(), 0);
    }

    #[test]
    fn test_deep_nesting_within_limit() {
        let db = create_test_database();
        let storage = create_test_storage();

        // Create a chain of 5 documents (within the 10-level limit)
        let root_id = insert_dummy_document(&db, &storage, "Doc 0", None);
        let mut current_id = root_id;

        for i in 1..=5 {
            let title = format!("Doc {i}");
            current_id = insert_dummy_document(
                &db,
                &storage,
                &title,
                Some(create_reply_reference(current_id)),
            );
        }

        // Should successfully retrieve the entire chain starting from the root
        let tree = db.get_reply_tree_for_document(root_id, &storage).unwrap();
        assert!(tree.is_some());

        // Walk down the chain to verify depth
        let mut current_tree = &tree.unwrap();
        let mut depth = 0;

        loop {
            depth += 1;
            if current_tree.replies.is_empty() {
                break;
            }
            current_tree = &current_tree.replies[0];
        }

        assert_eq!(depth, 6); // 6 total documents in chain (0 through 5)
    }

    #[test]
    fn test_mixed_scenario() {
        let db = create_test_database();
        let storage = create_test_storage();

        // Create complex tree: Root -> A, B; A -> A1; B -> B1, B2; B1 -> B1a
        let root = insert_dummy_document(&db, &storage, "Root", None);
        let doc_a = insert_dummy_document(&db, &storage, "A", Some(create_reply_reference(root)));
        let doc_b = insert_dummy_document(&db, &storage, "B", Some(create_reply_reference(root)));
        let _doc_a1 =
            insert_dummy_document(&db, &storage, "A1", Some(create_reply_reference(doc_a)));
        let doc_b1 =
            insert_dummy_document(&db, &storage, "B1", Some(create_reply_reference(doc_b)));
        let _doc_b2 =
            insert_dummy_document(&db, &storage, "B2", Some(create_reply_reference(doc_b)));
        let _doc_b1a =
            insert_dummy_document(&db, &storage, "B1a", Some(create_reply_reference(doc_b1)));

        let tree = db
            .get_reply_tree_for_document(root, &storage)
            .unwrap()
            .unwrap();

        // Root has 2 replies
        assert_eq!(tree.replies.len(), 2);

        // Find A and B branches
        let mut branch_a = None;
        let mut branch_b = None;
        for reply in &tree.replies {
            match reply.document.title.as_str() {
                "A" => branch_a = Some(reply),
                "B" => branch_b = Some(reply),
                _ => panic!("Unexpected reply: {}", reply.document.title),
            }
        }

        let branch_a = branch_a.unwrap();
        let branch_b = branch_b.unwrap();

        // A has 1 reply (A1)
        assert_eq!(branch_a.replies.len(), 1);
        assert_eq!(branch_a.replies[0].document.title, "A1");
        assert_eq!(branch_a.replies[0].replies.len(), 0); // A1 has no replies

        // B has 2 replies (B1, B2)
        assert_eq!(branch_b.replies.len(), 2);

        // Find B1 and B2
        let mut reply_b1 = None;
        let mut reply_b2 = None;
        for reply in &branch_b.replies {
            match reply.document.title.as_str() {
                "B1" => reply_b1 = Some(reply),
                "B2" => reply_b2 = Some(reply),
                _ => panic!("Unexpected B reply: {}", reply.document.title),
            }
        }

        let reply_b1 = reply_b1.unwrap();
        let reply_b2 = reply_b2.unwrap();

        // B1 has 1 reply (B1a), B2 has no replies
        assert_eq!(reply_b1.replies.len(), 1);
        assert_eq!(reply_b1.replies[0].document.title, "B1a");
        assert_eq!(reply_b2.replies.len(), 0);
    }
}
