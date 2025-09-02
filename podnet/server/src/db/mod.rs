use std::{collections::HashSet, sync::Mutex};

use hex::{FromHex, ToHex};
use pod2::{frontend::MainPod, middleware::Hash};
use podnet_models::{
    Document, DocumentContent, DocumentListItem, DocumentMetadata, DocumentPods, DocumentReplyTree,
    IdentityServer, Post, RawDocument, ReplyReference, Upvote, lazy_pod::LazyDeser,
};
use rusqlite::{Connection, OptionalExtension, Result};

pub mod migrations;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub async fn new(db_path: &str) -> anyhow::Result<Self> {
        let db_path = db_path.to_string();
        tokio::task::spawn_blocking(move || {
            let mut conn = Connection::open(&db_path)?;

            // --- Bootstrap logic for existing databases ---
            let current_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
            // TODO: We can eventually remove this once the production database is fully migrated.
            // Prior to adopting rusqlite_migration, we manually applied some migrations. We want
            // to still run this on existing databases to ensure they're up to date.
            if current_version == 0 {
                let table_info: String = conn
                    .query_row(
                        "SELECT sql FROM sqlite_master WHERE name = 'documents' AND type = 'table'",
                        [],
                        |row| row.get(0),
                    )
                    .optional()?
                    .unwrap_or_default();

                // We have a database which is at the baseline for version 7.
                if !table_info.is_empty() && table_info.contains("thread_root_id") {
                    tracing::info!(
                        "Detected existing un-versioned database. Baselining to latest migration version."
                    );
                    conn.execute_batch("PRAGMA user_version = 7")?;
                }
                // Otherwise, we should run migrations to reach the baseline.
            }

            migrations::MIGRATIONS.to_latest(&mut conn)?;

            let db = Database {
                conn: Mutex::new(conn),
            };
            Ok(db)
        })
        .await?
    }

    pub fn set_post_thread_links(
        &self,
        post_id: i64,
        parent_post_id: Option<i64>,
        thread_root_post_id: Option<i64>,
        reply_to_document_id: Option<i64>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE posts SET parent_post_id = ?1, thread_root_post_id = ?2, reply_to_document_id = ?3 WHERE id = ?4",
            rusqlite::params![parent_post_id, thread_root_post_id, reply_to_document_id, post_id],
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
        let mut stmt = conn.prepare(
            "SELECT id, created_at, last_edited_at, thread_root_post_id FROM posts WHERE id = ?1",
        )?;

        let post = stmt
            .query_row([id], |row| {
                Ok(Post {
                    id: Some(row.get(0)?),
                    created_at: Some(row.get(1)?),
                    last_edited_at: Some(row.get(2)?),
                    thread_root_post_id: Some(row.get(3)?),
                })
            })
            .optional()?;

        Ok(post)
    }

    pub fn get_all_posts(&self) -> Result<Vec<Post>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, created_at, last_edited_at, thread_root_post_id FROM posts ORDER BY last_edited_at DESC",
        )?;

        let posts = stmt
            .query_map([], |row| {
                Ok(Post {
                    id: Some(row.get(0)?),
                    created_at: Some(row.get(1)?),
                    last_edited_at: Some(row.get(2)?),
                    thread_root_post_id: Some(row.get(3)?),
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

        // Determine thread_root_id: use parent's thread root for replies, NULL for roots
        let thread_root_id: Option<i64> = if let Some(ref reply_ref) = reply_to {
            // This is a reply - get the thread_root_id from the parent document
            Some(
                tx.query_row(
                    "SELECT thread_root_id FROM documents WHERE id = ?1",
                    [reply_ref.document_id],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(|_| {
                    rusqlite::Error::InvalidColumnName("Parent document not found".to_string())
                })?,
            )
        } else {
            // Root document: set NULL initially to satisfy FK constraint, then update to self
            None
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
                thread_root_id, // Option<i64> -> NULL for roots, parent thread id for replies
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

        // If this is a root document (thread_root_id was NULL), update it to point to itself
        if thread_root_id.is_none() {
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
        // Query latest document per root post, capturing both new-model (post-based) and old-model (doc-based) latest reply
        type Row = (
            RawDocument,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        );

        let rows: Vec<Row> = {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT 
                    d.id, d.content_id, d.post_id, d.revision, d.created_at, d.pod, d.timestamp_pod,
                    d.uploader_id, d.upvote_count_pod, d.tags, d.authors, d.reply_to, d.requested_post_id, d.title,
                    -- New-model latest reply across descendant posts in this thread
                    (
                        SELECT MAX(r.created_at) FROM documents r
                        WHERE r.post_id IN (
                            SELECT c.id FROM posts c WHERE c.thread_root_post_id = p.id AND c.parent_post_id IS NOT NULL
                        )
                    ) AS latest_reply_at_new,
                    (
                        SELECT r.uploader_id FROM documents r
                        WHERE r.post_id IN (
                            SELECT c.id FROM posts c WHERE c.thread_root_post_id = p.id AND c.parent_post_id IS NOT NULL
                        )
                        ORDER BY r.created_at DESC LIMIT 1
                    ) AS latest_reply_by_new,
                    -- Old-model latest reply within the same post using document-level reply_to
                    (
                        SELECT MAX(rr.created_at) FROM documents rr WHERE rr.post_id = p.id AND rr.reply_to IS NOT NULL
                    ) AS latest_reply_at_old,
                    (
                        SELECT rr.uploader_id FROM documents rr WHERE rr.post_id = p.id AND rr.reply_to IS NOT NULL
                        ORDER BY rr.created_at DESC LIMIT 1
                    ) AS latest_reply_by_old
                 FROM posts p
                 JOIN documents d ON d.post_id = p.id AND d.revision = (
                    SELECT MAX(x.revision) FROM documents x WHERE x.post_id = p.id AND (x.reply_to IS NULL)
                 )
                 WHERE p.parent_post_id IS NULL
                 ORDER BY d.created_at DESC",
            )?;

            stmt.query_map([], |row| {
                // Parse fields for latest root document
                let tags_json: String = row.get(9)?;
                let tags: HashSet<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                let authors_json: String = row.get(10)?;
                let authors: HashSet<String> =
                    serde_json::from_str(&authors_json).unwrap_or_default();
                let reply_to_json: Option<String> = row.get(11)?;
                let reply_to: Option<ReplyReference> =
                    reply_to_json.and_then(|json| serde_json::from_str(&json).ok());

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

                let latest_reply_at_new: Option<String> = row.get(14)?;
                let latest_reply_by_new: Option<String> = row.get(15)?;
                let latest_reply_at_old: Option<String> = row.get(16)?;
                let latest_reply_by_old: Option<String> = row.get(17)?;

                Ok((
                    raw_doc,
                    latest_reply_at_new,
                    latest_reply_by_new,
                    latest_reply_at_old,
                    latest_reply_by_old,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?
        };

        // Now, outside of the DB lock, convert and choose latest between models
        let mut result = Vec::new();
        for (raw_doc, at_new, by_new, at_old, by_old) in rows {
            let metadata = self.raw_document_to_metadata(raw_doc)?;
            let (latest_reply_at, latest_reply_by) = match (at_new.as_ref(), at_old.as_ref()) {
                (Some(a), Some(b)) => {
                    if a >= b {
                        (at_new, by_new)
                    } else {
                        (at_old, by_old)
                    }
                }
                (Some(_), None) => (at_new, by_new),
                (None, Some(_)) => (at_old, by_old),
                (None, None) => (None, None),
            };

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

    /// Delete all documents in a post. Returns number of deleted documents.
    pub fn delete_documents_by_post_id(&self, post_id: i64) -> Result<usize> {
        let conn = self.conn.lock().unwrap();

        // Delete upvotes for documents in this post
        conn.execute(
            "DELETE FROM upvotes WHERE document_id IN (SELECT id FROM documents WHERE post_id = ?1)",
            [post_id],
        )?;

        // Delete documents in this post
        let deleted = conn.execute("DELETE FROM documents WHERE post_id = ?1", [post_id])?;

        Ok(deleted)
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

    // Get complete reply tree for a specific document using posts table hierarchy
    pub fn get_reply_tree_for_document(
        &self,
        document_id: i64,
        storage: &crate::storage::ContentAddressedStorage,
    ) -> Result<Option<DocumentReplyTree>> {
        // First get the post_id for this document
        let document_post_id = match self.get_document_post_id(document_id)? {
            Some(post_id) => post_id,
            None => return Ok(None), // Document doesn't exist
        };

        // Get the thread_root_post_id from the posts table
        let thread_root_post_id = match self.get_post_thread_root_id(document_post_id)? {
            Some(root_id) => root_id,
            None => {
                // If thread_root_post_id is not set, treat the post itself as the root
                tracing::debug!(
                    "Posts hierarchy not set for post {}, using post as thread root",
                    document_post_id
                );
                document_post_id
            }
        };

        // Get ALL documents in the thread including all revisions of all posts
        let all_thread_documents =
            self.get_all_documents_in_thread_with_revisions(thread_root_post_id)?;

        // Get the posts hierarchy for building the tree structure
        let posts_hierarchy = self.get_posts_hierarchy_for_thread(thread_root_post_id)?;

        // Build the tree structure using posts hierarchy, starting with the requested document
        self.build_reply_tree_from_posts_and_documents(
            all_thread_documents,
            posts_hierarchy,
            document_id,
            storage,
        )
    }

    // Helper method to get post_id for a document
    pub fn get_document_post_id(&self, document_id: i64) -> Result<Option<i64>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT post_id FROM documents WHERE id = ?1",
            [document_id],
            |row| row.get(0),
        )
        .optional()
    }

    // Helper method to get thread_root_post_id for a post
    pub fn get_post_thread_root_id(&self, post_id: i64) -> Result<Option<i64>> {
        let conn = self.conn.lock().unwrap();
        let result = conn
            .query_row(
                "SELECT thread_root_post_id FROM posts WHERE id = ?1",
                [post_id],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?;

        // If the row exists, extract the Option<i64>, otherwise return None
        Ok(result.flatten())
    }

    // Helper method to get thread_root_id for a document (kept for compatibility)
    pub fn get_document_thread_root_id(&self, document_id: i64) -> Result<Option<i64>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT thread_root_id FROM documents WHERE id = ?1",
            [document_id],
            |row| row.get(0),
        )
        .optional()
    }

    // Helper method to get all documents in a thread using posts table hierarchy
    pub fn get_all_documents_in_thread_with_revisions(
        &self,
        thread_root_post_id: i64,
    ) -> Result<Vec<RawDocument>> {
        let conn = self.conn.lock().unwrap();

        // Get all documents for all posts in this thread using posts table hierarchy
        let mut stmt = conn.prepare(
            "SELECT d.id, d.content_id, d.post_id, d.revision, d.created_at, d.pod, d.timestamp_pod, 
                    d.uploader_id, d.upvote_count_pod, d.tags, d.authors, d.reply_to, d.requested_post_id, d.title
             FROM posts p
             JOIN documents d ON p.id = d.post_id
             WHERE p.thread_root_post_id = ?1 OR p.id = ?1
             ORDER BY d.created_at ASC"
        )?;

        let documents = stmt
            .query_map([thread_root_post_id], |row| {
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

    // Helper method to get posts hierarchy information for thread building
    pub fn get_posts_hierarchy_for_thread(
        &self,
        thread_root_post_id: i64,
    ) -> Result<std::collections::HashMap<i64, Option<i64>>> {
        let conn = self.conn.lock().unwrap();

        // Get post_id -> parent_post_id mapping for all posts in the thread
        let mut stmt = conn.prepare(
            "SELECT id, parent_post_id FROM posts WHERE thread_root_post_id = ?1 OR id = ?1",
        )?;

        let mut post_hierarchy = std::collections::HashMap::new();
        let rows = stmt.query_map([thread_root_post_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?))
        })?;

        for row in rows {
            let (post_id, parent_post_id) = row?;
            post_hierarchy.insert(post_id, parent_post_id);
        }

        Ok(post_hierarchy)
    }

    // Helper method to get all documents in a thread (original method, kept for compatibility)
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

    // Helper method to build tree structure using posts hierarchy and documents
    fn build_reply_tree_from_posts_and_documents(
        &self,
        raw_documents: Vec<RawDocument>,
        posts_hierarchy: std::collections::HashMap<i64, Option<i64>>,
        requested_document_id: i64,
        storage: &crate::storage::ContentAddressedStorage,
    ) -> Result<Option<DocumentReplyTree>> {
        use std::collections::HashMap;

        if raw_documents.is_empty() {
            return Ok(None);
        }

        // Create mappings for building the tree
        let mut document_map: HashMap<i64, DocumentMetadata> = HashMap::new();
        let mut content_map: HashMap<i64, DocumentContent> = HashMap::new();
        let mut post_to_documents: HashMap<i64, Vec<i64>> = HashMap::new();

        // Process all documents
        for raw_doc in &raw_documents {
            let doc_id = raw_doc.id.unwrap_or(-1);
            let metadata = self.raw_document_to_metadata(raw_doc.clone())?;

            // Fetch content from storage
            let content_hash = Hash::from_hex(raw_doc.content_id.clone()).map_err(|_| {
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

            document_map.insert(doc_id, metadata);
            content_map.insert(doc_id, content);
            post_to_documents
                .entry(raw_doc.post_id)
                .or_default()
                .push(doc_id);
        }

        // Choose representative document for each post (prefer requested document, then latest revision)
        let mut post_representatives: HashMap<i64, i64> = HashMap::new();
        for (post_id, doc_ids) in &post_to_documents {
            let representative = if doc_ids.contains(&requested_document_id) {
                requested_document_id
            } else {
                // Find document with highest revision number
                doc_ids
                    .iter()
                    .map(|&doc_id| {
                        let raw_doc = raw_documents.iter().find(|d| d.id == Some(doc_id)).unwrap();
                        (doc_id, raw_doc.revision)
                    })
                    .max_by_key(|&(_, revision)| revision)
                    .map(|(doc_id, _)| doc_id)
                    .unwrap_or(*doc_ids.first().unwrap())
            };
            post_representatives.insert(*post_id, representative);
        }

        // Build parent-child relationships based on posts hierarchy
        let mut children_map: HashMap<i64, Vec<i64>> = HashMap::new();
        for (post_id, parent_post_id) in &posts_hierarchy {
            if let Some(parent_id) = parent_post_id {
                // This post is a reply to another post
                if let (Some(&child_doc), Some(&parent_doc)) = (
                    post_representatives.get(post_id),
                    post_representatives.get(parent_id),
                ) {
                    children_map.entry(parent_doc).or_default().push(child_doc);
                }
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
}
