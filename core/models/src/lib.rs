#![feature(stmt_expr_attributes)]

use std::collections::HashSet;

use hex::ToHex;
use lazy_pod::LazyDeser;
use pod2::{
    backends::plonky2::primitives::ec::curve::Point as PublicKey,
    frontend::{MainPod, SignedPod},
    middleware::{Hash, KEY_SIGNER, KEY_TYPE, Key, PodType, Value},
};
use serde::{Deserialize, Serialize};

/// Lazy deserialization wrappers for pods
pub mod lazy_pod;
pub mod macros;
/// Main pod operations and verification utilities
pub mod mainpod;

/// File attachment within a document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentFile {
    pub name: String,      // Original filename
    pub content: Vec<u8>,  // File bytes (base64 encoded in JSON)
    pub mime_type: String, // MIME type
}

/// Multi-content document structure supporting messages, files, and URLs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentContent {
    pub message: Option<String>,    // Text message
    pub file: Option<DocumentFile>, // File attachment
    pub url: Option<String>,        // URL reference
}

impl DocumentContent {
    /// Validate that at least one content type is provided
    pub fn validate(&self) -> Result<(), String> {
        if self.message.is_none() && self.file.is_none() && self.url.is_none() {
            return Err("At least one of message, file, or url must be provided".to_string());
        }

        // Validate file size (max 10MB)
        if let Some(ref file) = self.file {
            const MAX_FILE_SIZE: usize = 10 * 1024 * 1024; // 10MB
            if file.content.len() > MAX_FILE_SIZE {
                return Err(format!(
                    "File size {} exceeds maximum allowed size of {}",
                    file.content.len(),
                    MAX_FILE_SIZE
                ));
            }
        }

        // Validate URL format
        if let Some(ref url) = self.url {
            if !url.starts_with("http://") && !url.starts_with("https://") {
                return Err("URL must start with http:// or https://".to_string());
            }
        }

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Post {
    pub id: Option<i64>,
    pub created_at: Option<String>,
    pub last_edited_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawDocument {
    pub id: Option<i64>,
    pub content_id: String,
    pub post_id: i64,
    pub revision: i64,
    pub created_at: Option<String>,
    pub pod: String,                      // JSON string of the signed pod
    pub timestamp_pod: String,            // JSON string of the server timestamp pod
    pub uploader_id: String,              // Username of the uploader
    pub upvote_count_pod: Option<String>, // JSON string of the upvote count main pod
    pub tags: HashSet<String>,            // Set of tags for document organization
    pub authors: HashSet<String>,         // Set of authors for document attribution
    pub reply_to: Option<i64>,            // Document ID this document is replying to
    pub requested_post_id: Option<i64>,   // Original post_id from request used in MainPod proof
    pub title: String,                    // Document title
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PostWithDocuments {
    pub id: Option<i64>,
    pub created_at: Option<String>,
    pub last_edited_at: Option<String>,
    pub documents: Vec<DocumentMetadata>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub id: Option<i64>,
    pub content_id: Hash,
    pub post_id: i64,
    pub revision: i64,
    pub created_at: Option<String>,
    /// MainPod that proves:
    /// - Identity verification: identity pod was signed by registered identity server
    /// - Document verification: document pod was signed by user from identity pod
    /// - Cross verification: document signer matches identity user_public_key
    /// - Content hash verification: document pod contains correct content hash
    ///
    /// Public data exposed by main pod:
    /// - username: String (verified username from identity pod)
    /// - content_hash: String (verified Poseidon hash of content)
    /// - user_public_key: Point (verified user public key)
    /// - identity_server_pk: Point (verified identity server public key)
    pub pod: LazyDeser<MainPod>,
    /// SignedPod containing server timestamp information:
    /// - post_id: i64 (ID of the post this document belongs to)
    /// - document_id: i64 (ID of this document revision)
    /// - timestamp: i64 (server timestamp when document was created)
    /// - _signer: Point (server's public key, automatically added by SignedPod)
    ///
    /// This pod proves the document was timestamped by the server and establishes
    /// the canonical ordering of document creation.
    pub timestamp_pod: LazyDeser<SignedPod>,
    pub uploader_id: String, // Username of the uploader
    pub upvote_count: i64,   // Number of upvotes for this document
    /// MainPod that cryptographically proves the upvote count is correct
    /// Proves: upvote_count(N, content_hash, post_id) where N is the actual count
    /// Uses recursive proofs starting from base case (count=0) and building up
    pub upvote_count_pod: LazyDeser<Option<MainPod>>,
    pub tags: HashSet<String>, // Set of tags for document organization and discovery
    pub authors: HashSet<String>, // Set of authors for document attribution
    pub reply_to: Option<i64>, // Document ID this document is replying to
    /// Original post_id value from the publish request used in the MainPod proof
    /// This may be -1 for new documents, while post_id is the actual assigned ID
    pub requested_post_id: Option<i64>,
    pub title: String, // Document title
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Document {
    pub metadata: DocumentMetadata,
    pub content: DocumentContent, // Retrieved from storage
}

impl Document {
    /// Verify all cryptographic proofs for this document
    pub fn verify(&self, server_public_key: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Verify publish verification (MainPod)
        self.verify_publish_verification()?;

        // Verify timestamp pod signature
        self.verify_timestamp_pod_signature(server_public_key)?;

        // Verify upvote count pod if present
        self.verify_upvote_count_pod()?;

        println!("✓ All document verification checks passed");
        Ok(())
    }

    /// Verify the publish verification MainPod
    pub fn verify_publish_verification(&self) -> Result<(), Box<dyn std::error::Error>> {
        use std::collections::HashMap;

        use pod2::middleware::{
            Statement,
            containers::{Dictionary, Set},
        };

        println!("Verifying publish verification MainPod...");

        let main_pod = self.metadata.pod.get()?;

        // Extract identity server public key from MainPod
        let publish_verified_statement = &main_pod.public_statements[1];
        let identity_server_pk = match publish_verified_statement {
            Statement::Custom(_, args) => &args[2],
            _ => return Err("Invalid MainPod structure".into()),
        };

        // Build expected data dictionary
        let mut data_map = HashMap::new();
        data_map.insert(
            Key::from("content_hash"),
            Value::from(self.metadata.content_id),
        );

        // Convert tags to Set
        let tags_set = Set::new(
            5,
            self.metadata
                .tags
                .iter()
                .map(|tag| Value::from(tag.clone()))
                .collect::<HashSet<_>>(),
        )
        .map_err(|e| format!("Failed to create tags set: {:?}", e))?;
        data_map.insert(Key::from("tags"), Value::from(tags_set));

        // Convert authors to Set
        let authors_set = Set::new(
            5,
            self.metadata
                .authors
                .iter()
                .map(|author| Value::from(author.clone()))
                .collect::<HashSet<_>>(),
        )
        .map_err(|e| format!("Failed to create authors set: {:?}", e))?;
        data_map.insert(Key::from("authors"), Value::from(authors_set));

        // Add reply_to
        data_map.insert(
            Key::from("reply_to"),
            Value::from(self.metadata.reply_to.unwrap_or(-1)),
        );

        // Add post_id (use requested_post_id if available, otherwise use post_id)
        let verification_post_id = self
            .metadata
            .requested_post_id
            .unwrap_or(self.metadata.post_id);
        data_map.insert(Key::from("post_id"), Value::from(verification_post_id));

        // Create expected data dictionary
        let expected_data = Dictionary::new(6, data_map)
            .map_err(|e| format!("Failed to create expected data dictionary: {:?}", e))?;

        // Use solver-based verification
        mainpod::publish::verify_publish_verification_with_solver(
            main_pod,
            &self.metadata.uploader_id,
            &expected_data,
            identity_server_pk,
        )
        .map_err(|e| format!("Publish verification failed: {e}"))?;

        println!("✓ Publish verification successful");
        Ok(())
    }

    /// Verify the timestamp pod signature
    pub fn verify_timestamp_pod_signature(
        &self,
        server_public_key: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use pod2::frontend::SignedPod;

        println!("Verifying timestamp pod signature...");

        let timestamp_pod = self.metadata.timestamp_pod.get()?;

        // Verify signature
        timestamp_pod.verify()?;

        // TODO: get the server public key
        // Check that the signer matches the server public key
        //let pod_signer = timestamp_pod
        //    .get("_signer")
        //    .ok_or("Timestamp pod missing signer")?;
        //
        //let pod_signer_str = format!("{pod_signer}");
        //let server_public_key = format!("pk:{server_public_key}");
        //if pod_signer_str != server_public_key {
        //    return Err(format!(
        //        "Timestamp pod signer {pod_signer_str} does not match server public key {server_public_key}"
        //    ).into());
        //}

        println!("✓ Timestamp pod signature verified");
        Ok(())
    }

    /// Verify the upvote count pod if present
    pub fn verify_upvote_count_pod(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(upvote_count_pod) = self.metadata.upvote_count_pod.get()? {
            println!("Verifying upvote count pod...");

            mainpod::upvote::verify_upvote_count_with_solver(
                upvote_count_pod,
                self.metadata.upvote_count,
                &self.metadata.content_id,
            )
            .map_err(|e| format!("Upvote count verification failed: {e}"))?;

            println!(
                "✓ Upvote count verification successful (count: {})",
                self.metadata.upvote_count
            );
        } else if self.metadata.upvote_count > 0 {
            println!(
                "⚠️  Warning: Document claims {} upvotes but no upvote count proof provided",
                self.metadata.upvote_count
            );
        }

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PublishRequest {
    pub title: String, // Document title
    pub content: DocumentContent,
    pub tags: HashSet<String>,    // Set of tags for document organization
    pub authors: HashSet<String>, // Set of authors for document attribution
    pub reply_to: Option<i64>,    // Document ID this document is replying to
    pub post_id: Option<i64>,     // Post ID (None means create new post)
    pub username: String,         // Expected username from identity verification
    /// MainPod that cryptographically proves the user's identity and document authenticity:
    ///
    /// Uses the new solver-based approach with:
    /// - identity_verified(username, private: identity_pod)
    /// - publish_verified(username, data, identity_server_pk, private: identity_pod, document_pod)
    ///
    /// The MainPod proves:
    /// - Identity verification: identity pod was signed by registered identity server
    /// - Document verification: document pod was signed by user from identity pod
    /// - Cross verification: document signer matches identity user_public_key
    /// - Data verification: document pod contains correct content hash and metadata
    ///
    /// Public data exposed by main pod:
    /// - username: String (verified username from identity pod)
    /// - data: Dictionary (verified document data including content_hash, tags, authors, etc.)
    /// - identity_server_pk: Point (verified identity server public key)
    ///
    /// This enables trustless document publishing with verified authorship.
    pub main_pod: MainPod,
}

#[derive(Debug, Serialize)]
pub struct MarkdownResponse {
    pub html: String,
}

#[derive(Debug, Serialize)]
pub struct ServerInfo {
    pub public_key: PublicKey,
}

#[derive(Debug, Deserialize)]
pub struct UserRegistration {
    pub user_id: String,
    pub public_key: PublicKey,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub id: Option<i64>,
    pub user_id: String,
    pub public_key: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IdentityServer {
    pub id: Option<i64>,
    pub server_id: String,
    pub public_key: String,    // Stored as string in DB
    pub challenge_pod: String, // Server's challenge pod as JSON string
    pub identity_pod: String,  // Identity server's response pod as JSON string
    pub created_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct IdentityServerChallengeRequest {
    /// Request from identity server to get a challenge for registration
    pub server_id: String,
    pub public_key: PublicKey,
}

#[derive(Debug, Serialize)]
pub struct IdentityServerChallengeResponse {
    /// SignedPod containing challenge information from main server:
    /// - challenge: String (random challenge value)
    /// - expires_at: String (ISO timestamp when challenge expires)
    /// - identity_server_public_key: Point (public key from request)
    /// - server_id: String (server ID from request)
    /// - _signer: Point (main server's public key, automatically added by SignedPod)
    pub challenge_pod: SignedPod,
}

#[derive(Debug, Deserialize)]
pub struct IdentityServerRegistration {
    /// Registration request containing both server's challenge and identity server's response
    ///
    /// server_challenge_pod contains:
    /// - challenge: String (original challenge from server)
    /// - expires_at: String (expiration timestamp)
    /// - identity_server_public_key: Point (identity server's public key)
    /// - server_id: String (identity server ID)
    /// - _signer: Point (main server's public key)
    ///
    /// identity_response_pod contains:
    /// - challenge: String (same challenge value, proving identity server received it)
    /// - server_id: String (confirming identity server ID)
    /// - _signer: Point (identity server's public key, proving control of private key)
    pub server_challenge_pod: SignedPod,
    pub identity_response_pod: SignedPod,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Upvote {
    pub id: Option<i64>,
    pub document_id: i64,
    pub username: String,
    pub pod_json: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpvoteRequest {
    pub username: String, // Expected username from identity verification
    /// MainPod that cryptographically proves the user's identity and upvote authenticity:
    ///
    /// Uses the new solver-based approach with:
    /// - identity_verified(username, private: identity_pod)
    /// - upvote_verified(content_hash, private: upvote_pod)
    /// - upvote_verification(username, content_hash, identity_server_pk, private: identity_pod, upvote_pod)
    ///
    /// The MainPod proves:
    /// - Identity verification: identity pod was signed by registered identity server
    /// - Upvote verification: upvote pod was signed by user from identity pod
    /// - Cross verification: upvote signer matches identity user_public_key
    /// - Document hash verification: upvote pod contains correct document content hash
    /// - Request type verification: upvote pod specifies "upvote" request type
    ///
    /// Public data exposed by main pod:
    /// - username: String (verified username from identity pod)
    /// - content_hash: String (verified content hash of upvoted document)
    /// - identity_server_pk: Point (verified identity server public key)
    ///
    /// This enables trustless upvoting with verified user identity.
    pub upvote_main_pod: MainPod,
}

/// Shared predicate definitions for publish verification
pub fn get_publish_verification_predicate() -> String {
    // TODO: is there a better strategy for many args in the predicates?
    //
    //    document_verified(content_hash, post_id, tags, authors, reply_to, private: document_pod) = AND(
    //        Equal(?document_pod["{key_type}"], {signed_pod_type})
    //        Equal(?document_pod["content_hash"], ?content_hash)
    //        Equal(?document_pod["tags"], ?tags)
    //        Equal(?document_pod["authors"], ?authors)
    //        Equal(?document_pod["post_id"], ?post_id)
    //        Equal(?document_pod["reply_to"], ?reply_to)
    //    )

    //    publish_verification(username, content_hash, identity_server_pk, post_id, tags, authors, reply_to, private: identity_pod, document_pod) = AND(
    //        identity_verified(?username)
    //        document_verified(?content_hash, ?post_id, ?tags, ?authors, ?reply_to)
    //        Equal(?identity_pod["{key_signer}"], ?identity_server_pk)
    //        Equal(?identity_pod["user_public_key"], ?document_pod["{key_signer}"])
    //    )
    format!(
        r#"
        identity_verified(username, private: identity_pod) = AND(
            Equal(?identity_pod["{key_type}"], {signed_pod_type})
            Equal(?identity_pod["username"], ?username)
        )

        publish_verified(username, data, identity_server_pk, private: identity_pod, document_pod) = AND(
            identity_verified(?username)
            Equal(?document_pod["request_type"], "publish")
            Equal(?document_pod["data"], ?data)
            Equal(?document_pod["{key_signer}"], ?identity_pod["user_public_key"])
            Equal(?identity_pod["{key_signer}"], ?identity_server_pk)
        )
        "#,
        key_type = KEY_TYPE,
        key_signer = KEY_SIGNER,
        signed_pod_type = PodType::Signed as usize,
    )
}

/// Shared predicate definitions for upvote verification only
pub fn get_upvote_verification_predicate() -> String {
    format!(
        r#"
        identity_verified(username, private: identity_pod) = AND(
            Equal(?identity_pod["{key_type}"], {signed_pod_type})
            Equal(?identity_pod["username"], ?username)
        )

        upvote_verified(content_hash, private: upvote_pod) = AND(
            Equal(?upvote_pod["{key_type}"], {signed_pod_type})
            Equal(?upvote_pod["content_hash"], ?content_hash)
            Equal(?upvote_pod["request_type"], "upvote")
        )

        upvote_verification(username, content_hash, identity_server_pk, private: identity_pod, upvote_pod) = AND(
            identity_verified(?username)
            upvote_verified(?content_hash)
            Equal(?identity_pod["{key_signer}"], ?identity_server_pk)
            Equal(?identity_pod["user_public_key"], ?upvote_pod["{key_signer}"])
        )
        "#,
        key_type = KEY_TYPE,
        key_signer = KEY_SIGNER,
        signed_pod_type = PodType::Signed as usize,
    )
}

/// Shared predicate definitions for upvote count verification only
pub fn get_upvote_count_predicate(upvote_batch_id: Hash) -> String {
    format!(
        r#"
        use _, _, upvote_verification from 0x{id}

        upvote_count_base(count, content_hash, private: data_pod) = AND(
            Equal(?count, 0)
            Equal(?data_pod["content_hash"], ?content_hash)
        )

        upvote_count_ind(count, content_hash, private: intermed, username, identity_server_pk) = AND(
            upvote_count(?intermed, ?content_hash)
            SumOf(?count, ?intermed, 1)
            upvote_verification(?username, ?content_hash, ?identity_server_pk)
            Lt(0, ?count)
        )

        upvote_count(count, content_hash) = OR(
            upvote_count_base(?count, ?content_hash)
            upvote_count_ind(?count, ?content_hash)
        )
        "#,
        id = upvote_batch_id.encode_hex::<String>(),
    )
}
