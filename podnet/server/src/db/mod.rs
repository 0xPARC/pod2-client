use std::{collections::HashSet, sync::Mutex};

use hex::{FromHex, ToHex};
use pod2::{frontend::MainPod, middleware::Hash};
use podnet_models::{
    Document, DocumentMetadata, IdentityServer, Post, RawDocument, Upvote, lazy_pod::LazyDeser,
};
use rusqlite::{Connection, OptionalExtension, Result};

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub async fn new(db_path: &str) -> anyhow::Result<Self> {
        let conn = Connection::open(db_path)?;
        let db = Database {
            conn: Mutex::new(conn),
        };
        db.init_tables()?;
        Ok(db)
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
                reply_to INTEGER,
                requested_post_id INTEGER,
                title TEXT NOT NULL,
                FOREIGN KEY (post_id) REFERENCES posts (id),
                FOREIGN KEY (reply_to) REFERENCES documents (id),
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

    pub fn update_post_last_edited(&self, post_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE posts SET last_edited_at = CURRENT_TIMESTAMP WHERE id = ?1",
            [post_id],
        )?;
        Ok(())
    }

    // Document methods
    pub fn create_document(
        &self,
        content_id: &Hash,
        post_id: i64,
        pod: &MainPod,
        uploader_id: &str,
        tags: &HashSet<String>,
        authors: &HashSet<String>,
        reply_to: Option<i64>,
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

        // Insert document with empty timestamp_pod and null upvote_count_pod initially
        tx.execute(
            "INSERT INTO documents (content_id, post_id, revision, pod, timestamp_pod, uploader_id, upvote_count_pod, tags, authors, reply_to, requested_post_id, title) VALUES (?1, ?2, ?3, ?4, '', ?5, NULL, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                content_id_string,
                post_id,
                next_revision,
                pod_json,
                uploader_id,
                tags_json,
                authors_json,
                reply_to,
                requested_post_id,
                title,
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

        // Create the metadata with properly typed pods
        let metadata = DocumentMetadata {
            id: Some(document_id),
            content_id: *content_id,
            post_id,
            revision: next_revision,
            created_at: None, // Will be filled by database
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
            uploader_id: uploader_id.to_string(),
            upvote_count,
            upvote_count_pod: LazyDeser::from_value(None::<MainPod>).map_err(|_| {
                rusqlite::Error::InvalidColumnType(
                    0,
                    "upvote_count_pod".to_string(),
                    rusqlite::types::Type::Text,
                )
            })?, // Will be set by background task
            tags: tags.clone(),
            authors: authors.clone(),
            reply_to,
            requested_post_id,
            title: title.to_string(),
        };

        Ok(Document { metadata, content })
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
                    reply_to: row.get(11)?,
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
                    reply_to: row.get(11)?,
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
                    reply_to: row.get(11)?,
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
                    reply_to: row.get(11)?,
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

    // Helper method to convert RawDocument to DocumentMetadata
    pub fn raw_document_to_metadata(&self, raw_doc: RawDocument) -> Result<DocumentMetadata> {
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
            pod,
            timestamp_pod,
            uploader_id: raw_doc.uploader_id,
            upvote_count,
            upvote_count_pod,
            tags: raw_doc.tags,
            authors: raw_doc.authors,
            reply_to: raw_doc.reply_to,
            requested_post_id: raw_doc.requested_post_id,
            title: raw_doc.title,
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

                Ok(Some(Document { metadata, content }))
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
             FROM documents WHERE reply_to = ?1 ORDER BY created_at ASC",
        )?;

        let documents = stmt
            .query_map([document_id], |row| {
                let tags_json: String = row.get(9)?;
                let tags: HashSet<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                let authors_json: String = row.get(10)?;
                let authors: HashSet<String> =
                    serde_json::from_str(&authors_json).unwrap_or_default();
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
                    reply_to: row.get(11)?,
                    requested_post_id: row.get(12)?,
                    title: row.get(13)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(documents)
    }
}
