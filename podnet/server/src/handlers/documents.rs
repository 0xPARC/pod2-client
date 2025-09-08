use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
};
use pod2::middleware::{
    Key, Value,
    containers::{Dictionary, Set},
};
use podnet_models::{
    DeleteRequest, Document, DocumentMetadata, PublishRequest,
    mainpod::{
        delete::verify_delete_verification_with_solver,
        publish::verify_publish_verification_with_solver,
    },
};

pub async fn get_documents(
    State(state): State<Arc<crate::AppState>>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    /*    // Extract conditional request header
        let if_modified_since = headers.get(header::IF_MODIFIED_SINCE);

        // Get the most recent modification time for cache validation
        let last_modified = state
            .db
            .get_most_recent_modification_time()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let mut headers = HeaderMap::new();

        if let Some(last_modified_str) = &last_modified {
            // Convert SQLite timestamp to HTTP date format
            if let Ok(last_modified_time) =
                chrono::NaiveDateTime::parse_from_str(last_modified_str, "%Y-%m-%d %H:%M:%S")
            {
                let last_modified_utc: chrono::DateTime<chrono::Utc> =
                    chrono::DateTime::from_naive_utc_and_offset(last_modified_time, chrono::Utc);
                let http_date = last_modified_utc.to_rfc2822();

                // Set Last-Modified header in proper HTTP date format
                if let Ok(header_value) = HeaderValue::from_str(&http_date) {
                    headers.insert("last-modified", header_value);
                }
            }

            // Check If-Modified-Since header (time-based conditional request)
            if let Some(if_modified_since_value) = if_modified_since
                && let Ok(if_modified_since_str) = if_modified_since_value.to_str()
            {
                // Parse the last_modified timestamp from SQLite format
                if let Ok(last_modified_time) =
                    chrono::NaiveDateTime::parse_from_str(last_modified_str, "%Y-%m-%d %H:%M:%S")
                {
                    let last_modified_utc: chrono::DateTime<chrono::Utc> =
                        chrono::DateTime::from_naive_utc_and_offset(last_modified_time, chrono::Utc);

                    // Parse the client's If-Modified-Since header
                    if let Ok(client_time) = chrono::DateTime::parse_from_rfc2822(if_modified_since_str)
                    {
                        let client_time_utc = client_time.with_timezone(&chrono::Utc);

                        // Compare timestamps - if content hasn't been modified since client's timestamp
                        let is_modified = last_modified_utc > client_time_utc;

                        if !is_modified {
                            // Content not modified, return 304 Not Modified
                            return Ok(Response::builder()
                                .status(StatusCode::NOT_MODIFIED)
                                .body(axum::body::Body::empty())
                                .unwrap());
                        }
                    }
                }
            }
        }
    */
    // Content has been modified or no conditional headers, return full response
    // Fetch only top-level documents with latest reply info
    let documents_list = state
        .db
        .get_top_level_documents_with_latest_reply()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Set Cache-Control to allow caching but require validation
    /*   if let Ok(cache_control) = HeaderValue::from_str("public, max-age=0, must-revalidate") {
        headers.insert("cache-control", cache_control);
    } */

    Ok((headers, Json(documents_list)).into_response())
}

async fn get_document_from_db(
    document_id: i64,
    state: Arc<crate::AppState>,
) -> Result<Document, StatusCode> {
    let document = state
        .db
        .get_document(document_id, &state.storage)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(document)
}

pub async fn get_document_by_id(
    Path(id): Path<i64>,
    State(state): State<Arc<crate::AppState>>,
) -> Result<Json<Document>, StatusCode> {
    let document = get_document_from_db(id, state).await?;
    Ok(Json(document))
}

pub async fn publish_document(
    State(state): State<Arc<crate::AppState>>,
    Json(payload): Json<PublishRequest>,
) -> Result<Json<Document>, StatusCode> {
    tracing::info!("Starting document publish with main pod verification");

    // Validate the document content
    payload.content.validate().map_err(|e| {
        tracing::error!("Document content validation failed: {e}");
        StatusCode::BAD_REQUEST
    })?;
    tracing::info!("✓ Document content validated");

    // Validate reply content restrictions
    if payload.reply_to.is_some() {
        // Replies can only be messages, not files or URLs
        if payload.content.file.is_some() {
            tracing::error!("Replies cannot contain file attachments");
            return Err(StatusCode::BAD_REQUEST);
        }
        if payload.content.url.is_some() {
            tracing::error!("Replies cannot contain URLs");
            return Err(StatusCode::BAD_REQUEST);
        }
        if payload.content.message.is_none() {
            tracing::error!("Replies must contain a message");
            return Err(StatusCode::BAD_REQUEST);
        }
        tracing::info!("✓ Reply content restrictions validated");
    }

    // Validate the title
    if payload.title.trim().is_empty() {
        tracing::error!("Document title cannot be empty");
        return Err(StatusCode::BAD_REQUEST);
    }
    tracing::info!("✓ Document title validated");

    let (_vd_set, _prover) = state.pod_config.get_prover_setup()?;

    // Verify main pod proof
    tracing::info!("Verifying main pod proof");
    payload.main_pod.pod.verify().map_err(|e| {
        tracing::error!("Failed to verify main pod: {e}");
        StatusCode::UNAUTHORIZED
    })?;
    tracing::info!("✓ Main pod proof verified");

    // Store the content first to get its hash for verification
    tracing::info!("Storing content in content-addressed storage");
    let stored_content_hash = state
        .storage
        .store_document_content(&payload.content)
        .map_err(|e| {
            tracing::error!("Failed to store content: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    tracing::info!("Content stored successfully with hash: {stored_content_hash}");

    // Create the expected data structure for verification using request data
    tracing::info!("Creating expected data structure for solver verification");
    let mut data_map = HashMap::new();
    data_map.insert(Key::from("content_hash"), Value::from(stored_content_hash));

    // Convert tags HashSet to Set
    let tags_set = Set::new(
        5,
        payload
            .tags
            .iter()
            .map(|tag| Value::from(tag.clone()))
            .collect(),
    )
    .map_err(|e| {
        tracing::error!("Failed to create tags set: {e:?}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    data_map.insert(Key::from("tags"), Value::from(tags_set));

    // Convert authors HashSet to Set
    let authors_set = Set::new(
        5,
        payload
            .authors
            .iter()
            .map(|author| Value::from(author.clone()))
            .collect(),
    )
    .map_err(|e| {
        tracing::error!("Failed to create authors set: {e:?}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    data_map.insert(Key::from("authors"), Value::from(authors_set));

    // Add reply_to (convert ReplyReference to dictionary or use -1 if None)
    if let Some(ref reply_ref) = payload.reply_to {
        let mut reply_map = HashMap::new();
        reply_map.insert(Key::from("post_id"), Value::from(reply_ref.post_id));
        reply_map.insert(Key::from("document_id"), Value::from(reply_ref.document_id));
        let reply_dict = Dictionary::new(2, reply_map).map_err(|e| {
            tracing::error!("Failed to create reply_to dictionary: {e:?}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        data_map.insert(Key::from("reply_to"), Value::from(reply_dict));
    } else {
        data_map.insert(Key::from("reply_to"), Value::from(-1i64));
    }

    // Add post_id to data dictionary
    data_map.insert(
        Key::from("post_id"),
        match payload.post_id {
            Some(id) => Value::from(id),
            None => Value::from(-1i64), // Use -1 for None to match original logic
        },
    );

    // Create expected data dictionary
    let expected_data = Dictionary::new(6, data_map).map_err(|e| {
        tracing::error!("Failed to create expected data dictionary: {e:?}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // We need to first verify with all registered identity servers, since we don't know which one was used
    tracing::info!("Getting all registered identity servers for verification");
    let identity_servers = state.db.get_all_identity_servers().map_err(|e| {
        tracing::error!("Database error retrieving identity servers: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if identity_servers.is_empty() {
        tracing::error!("No identity servers registered");
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Try verification with each registered identity server until one succeeds
    let mut verification_succeeded = false;
    let mut identity_server_pk = None;

    for identity_server in &identity_servers {
        // Parse the identity server public key from database
        let server_pk: pod2::backends::plonky2::primitives::ec::curve::Point =
            serde_json::from_str(&identity_server.public_key).map_err(|e| {
                tracing::error!("Failed to parse identity server public key: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let server_pk_value = Value::from(server_pk);

        // Try verification with this identity server
        tracing::info!(
            "Trying verification with identity server: {}",
            identity_server.server_id
        );
        match verify_publish_verification_with_solver(
            &payload.main_pod,
            &payload.username,
            &expected_data,
            &server_pk_value,
        ) {
            Ok(_) => {
                tracing::info!(
                    "✓ Solver verification succeeded with identity server: {}",
                    identity_server.server_id
                );
                verification_succeeded = true;
                identity_server_pk = Some(server_pk);
                break;
            }
            Err(_) => {
                tracing::debug!(
                    "Verification failed with identity server: {}",
                    identity_server.server_id
                );
                continue;
            }
        }
    }

    if !verification_succeeded {
        tracing::error!("Solver-based verification failed with all registered identity servers");
        return Err(StatusCode::BAD_REQUEST);
    }

    let _identity_server_pk = identity_server_pk.unwrap();

    tracing::info!(
        "✓ Solver verification passed: username={}, content_hash={stored_content_hash}",
        payload.username
    );

    // Use the data from the request for further processing
    let uploader_username = &payload.username;
    let post_id = payload.post_id.unwrap_or(-1);
    let content_hash = stored_content_hash;

    // Identity server verification was already done above during solver verification

    // Determine post_id: either create new post or use existing
    tracing::info!("Determining post ID");
    // Determine final_post_id with new thread model:
    // - For replies: always create a new post that replies to the target's post
    // - For non-replies: use existing post_id for revisions, or create a new root post
    let final_post_id = if let Some(ref reply_ref) = payload.reply_to {
        tracing::info!(
            "Creating new reply post to post {} via document {}",
            reply_ref.post_id,
            reply_ref.document_id
        );
        // Verify parent target exists and get its metadata
        let target_doc = state
            .db
            .get_document_metadata(reply_ref.document_id)
            .map_err(|e| {
                tracing::error!(
                    "Database error checking reply_to document {}: {e}",
                    reply_ref.document_id
                );
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .ok_or_else(|| {
                tracing::error!("Reply_to document {} not found", reply_ref.document_id);
                StatusCode::NOT_FOUND
            })?;
        if target_doc.post_id != reply_ref.post_id {
            tracing::error!(
                "Reply_to post_id {} doesn't match document's actual post_id {}",
                reply_ref.post_id,
                target_doc.post_id
            );
            return Err(StatusCode::BAD_REQUEST);
        }
        // Create a new post for the reply
        let new_post_id = state.db.create_post().map_err(|e| {
            tracing::error!("Failed to create reply post: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        // Set thread links: parent = target_doc.post_id, thread_root = parent's root or parent, reply_to_document_id = reply_ref.document_id
        // Try to read parent's thread_root_post_id; if not set, use parent id
        let parent_post = state
            .db
            .get_post(target_doc.post_id)
            .map_err(|e| {
                tracing::error!("Failed to read parent post {}: {e}", target_doc.post_id);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .ok_or(StatusCode::NOT_FOUND)?;
        let thread_root_post_id = parent_post
            .thread_root_post_id
            .unwrap_or(parent_post.id.unwrap());
        state
            .db
            .set_post_thread_links(
                new_post_id,
                Some(target_doc.post_id),
                Some(thread_root_post_id),
                Some(reply_ref.document_id),
            )
            .map_err(|e| {
                tracing::error!("Failed to set thread links for reply post: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        tracing::info!("Reply post created with ID: {new_post_id}");
        new_post_id
    } else if post_id != -1 {
        tracing::info!("Using existing post ID for new revision: {post_id}");
        // Verify the post exists
        state
            .db
            .get_post(post_id)
            .map_err(|e| {
                tracing::error!("Database error checking post {post_id}: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .ok_or_else(|| {
                tracing::error!("Post {post_id} not found");
                StatusCode::NOT_FOUND
            })?;
        post_id
    } else {
        tracing::info!("Creating new root post");
        let id = state.db.create_post().map_err(|e| {
            tracing::error!("Failed to create new post: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        // For root posts, set thread_root_post_id to itself
        state
            .db
            .set_post_thread_links(id, None, Some(id), None)
            .map_err(|e| {
                tracing::error!("Failed to set thread links for root post: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        tracing::info!("New root post created with ID: {id}");
        id
    };

    // Validate reply_to if provided
    if let Some(ref reply_ref) = payload.reply_to {
        tracing::info!(
            "Validating reply_to document ID: {} in post: {}",
            reply_ref.document_id,
            reply_ref.post_id
        );

        // Verify the document being replied to exists
        let target_doc = state
            .db
            .get_document_metadata(reply_ref.document_id)
            .map_err(|e| {
                tracing::error!(
                    "Database error checking reply_to document {}: {e}",
                    reply_ref.document_id
                );
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .ok_or_else(|| {
                tracing::error!("Reply_to document {} not found", reply_ref.document_id);
                StatusCode::NOT_FOUND
            })?;

        // Verify the post_id matches
        if target_doc.post_id != reply_ref.post_id {
            tracing::error!(
                "Reply_to post_id {} doesn't match document's actual post_id {}",
                reply_ref.post_id,
                target_doc.post_id
            );
            return Err(StatusCode::BAD_REQUEST);
        }

        tracing::info!(
            "Reply_to reference validated: document {} in post {}",
            reply_ref.document_id,
            reply_ref.post_id
        );
    }

    // Create document with timestamp pod in a single transaction
    tracing::info!("Creating document for post {final_post_id}");
    let document = state
        .db
        .create_document(
            &content_hash,
            final_post_id,
            &payload.main_pod,
            uploader_username,
            &payload.tags,
            &payload.authors,
            payload.reply_to.clone(),
            Some(post_id), // Store original requested post_id for verification
            &payload.title,
            &state.storage,
        )
        .map_err(|e| {
            tracing::error!("Failed to create document: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    tracing::info!("Document created with ID: {:?}", document.metadata.id);

    // // Spawn background task to generate base case upvote count pod
    if let Some(document_id) = document.metadata.id {
        let state_clone = state.clone();
        let content_hash = document.metadata.content_id;

        tokio::spawn(async move {
            if let Err(e) = super::upvotes::generate_base_case_upvote_pod(
                state_clone,
                document_id,
                &content_hash,
            )
            .await
            {
                tracing::error!(
                    "Failed to generate base case upvote count pod for document {document_id}: {e}"
                );
            }
        });
    }

    // tracing::info!("Document publish completed successfully using main pod verification");
    Ok(Json(document))
}

pub async fn get_document_replies(
    Path(id): Path<i64>,
    State(state): State<Arc<crate::AppState>>,
) -> Result<Json<Vec<DocumentMetadata>>, StatusCode> {
    let raw_replies = state
        .db
        .get_replies_to_document(id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut replies = Vec::new();
    for raw_reply in raw_replies {
        let reply_metadata = state
            .db
            .raw_document_to_metadata(raw_reply)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        replies.push(reply_metadata);
    }

    Ok(Json(replies))
}

pub async fn get_document_reply_tree(
    Path(id): Path<i64>,
    State(state): State<Arc<crate::AppState>>,
) -> Result<Json<podnet_models::DocumentReplyTree>, StatusCode> {
    let reply_tree = state
        .db
        .get_reply_tree_for_document(id, &state.storage)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(reply_tree))
}

pub async fn delete_document(
    Path(id): Path<i64>,
    State(state): State<Arc<crate::AppState>>,
    Json(payload): Json<DeleteRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    tracing::info!("Starting document deletion with main pod verification for document {id}");

    // Verify the document ID matches the request
    if payload.document_id != id {
        tracing::error!(
            "Document ID mismatch: path {} vs payload {}",
            id,
            payload.document_id
        );
        return Err(StatusCode::BAD_REQUEST);
    }

    println!("GOt mainpod: {}", payload.main_pod);
    // Verify main pod proof
    tracing::info!("Verifying main pod proof for deletion");
    payload.main_pod.pod.verify().map_err(|e| {
        tracing::error!("Failed to verify main pod: {e}");
        StatusCode::UNAUTHORIZED
    })?;
    tracing::info!("✓ Main pod proof verified");

    // Check if document exists and get uploader info
    tracing::info!("Checking document exists and getting uploader info");
    let document = state
        .db
        .get_document(id, &state.storage)
        .map_err(|e| {
            tracing::error!("Database error retrieving document {id}: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;
    tracing::info!(
        "Document {} found, uploader: {}",
        id,
        document.metadata.uploader_id
    );

    // Verify username matches document uploader
    if payload.username != document.metadata.uploader_id {
        tracing::error!(
            "Username mismatch: requester '{}' vs document uploader '{}'",
            payload.username,
            document.metadata.uploader_id
        );
        return Err(StatusCode::FORBIDDEN);
    }
    tracing::info!("✓ Username verification passed");

    // Get all registered identity servers for verification
    tracing::info!("Getting all registered identity servers for verification");
    let identity_servers = state.db.get_all_identity_servers().map_err(|e| {
        tracing::error!("Database error retrieving identity servers: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if identity_servers.is_empty() {
        tracing::error!("No identity servers registered");
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Try verification with each registered identity server until one succeeds
    let mut verification_succeeded = false;

    let timestamp_pod = document
        .pods
        .timestamp_pod
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    tracing::info!("Got timestamp pod for document deletion verification: {timestamp_pod}");

    // Extract the original data from the publish MainPod
    let publish_main_pod = document.pods.pod.get().map_err(|e| {
        tracing::error!("Failed to get publish MainPod: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // The publish MainPod contains the verified data structure - extract it
    let publish_verified_statement = &publish_main_pod.public_statements[1]; // publish_verified statement
    let original_data = match publish_verified_statement {
        pod2::middleware::Statement::Custom(_, args) => &args[1], // Second argument is the data
        _ => {
            tracing::error!("Invalid MainPod structure - expected publish_verified statement");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    tracing::info!("✓ Original document data extracted from publish MainPod");

    for identity_server in &identity_servers {
        // Parse the identity server public key from database
        let server_pk: pod2::backends::plonky2::primitives::ec::curve::Point =
            serde_json::from_str(&identity_server.public_key).map_err(|e| {
                tracing::error!("Failed to parse identity server public key: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let server_pk_value = Value::from(server_pk);

        // Try verification with this identity server
        tracing::info!(
            "Trying deletion verification with identity server: {}",
            identity_server.server_id
        );
        match verify_delete_verification_with_solver(
            &payload.main_pod,
            &payload.username,
            original_data,
            &server_pk_value,
            timestamp_pod,
        ) {
            Ok(_) => {
                tracing::info!(
                    "✓ Solver verification succeeded with identity server: {}",
                    identity_server.server_id
                );
                verification_succeeded = true;
                break;
            }
            Err(_) => {
                tracing::debug!(
                    "Verification failed with identity server: {}",
                    identity_server.server_id
                );
                continue;
            }
        }
    }

    if !verification_succeeded {
        tracing::error!("Solver-based verification failed with all registered identity servers");
        return Err(StatusCode::UNAUTHORIZED);
    }

    tracing::info!(
        "✓ Solver verification passed: username={}, document_id={}",
        payload.username,
        payload.document_id
    );

    // Delete all documents in this post (temporary behavior)
    tracing::info!(
        "Deleting all documents in post {} (requested by delete of document {})",
        document.metadata.post_id,
        id
    );
    let deleted_uploader = document.metadata.uploader_id.clone();
    let _deleted_count = state
        .db
        .delete_documents_by_post_id(document.metadata.post_id)
        .map_err(|e| {
            tracing::error!(
                "Failed to delete documents for post {}: {}",
                document.metadata.post_id,
                e
            );
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    tracing::info!("Document deletion completed successfully for document {id}");

    Ok(Json(serde_json::json!({
        "success": true,
        "document_id": id,
        "deleted_by": payload.username,
        "original_uploader": deleted_uploader
    })))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{extract::Path, http::StatusCode};

    use super::*;
    use crate::db::Database;

    // Mock AppState for testing
    async fn create_mock_app_state() -> Arc<crate::AppState> {
        let db = Arc::new(
            Database::new(":memory:")
                .await
                .expect("Failed to create test database"),
        );

        // Create minimal storage and config for testing
        let storage =
            Arc::new(crate::storage::ContentAddressedStorage::new("/tmp/test_storage").unwrap());
        let config = crate::config::ServerConfig::load();
        let pod_config = crate::pod::PodConfig::new(true); // Use mock proofs

        Arc::new(crate::AppState {
            db,
            storage,
            config,
            pod_config,
        })
    }

    #[tokio::test]
    async fn test_get_document_reply_tree_success() {
        use crate::db::tests::insert_dummy_document;

        let state = create_mock_app_state().await;

        // Insert a test document using the test helper from db module
        let doc_id = insert_dummy_document(&state.db, &state.storage, "Test Document", None);

        // Call the handler
        let result = get_document_reply_tree(Path(doc_id), axum::extract::State(state)).await;

        // Verify success response
        assert!(result.is_ok());
        let response = result.unwrap();

        // Extract the tree from the JSON response
        let tree = response.0;
        assert_eq!(tree.document.title, "Test Document");
        assert_eq!(tree.replies.len(), 0); // No replies in this test
    }

    #[tokio::test]
    async fn test_get_document_reply_tree_not_found() {
        let state = create_mock_app_state().await;

        // Call the handler with a non-existent document ID
        let result = get_document_reply_tree(Path(99999), axum::extract::State(state)).await;

        // Verify 404 response
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error, StatusCode::NOT_FOUND);
    }

    // Test the existing get_document_replies handler for comparison
    #[tokio::test]
    async fn test_get_document_replies_success() {
        use crate::db::tests::{create_reply_reference, insert_dummy_document};

        let state = create_mock_app_state().await;

        // Create a document with replies using test helpers
        let root_id = insert_dummy_document(&state.db, &state.storage, "Root Document", None);
        let _reply_id = insert_dummy_document(
            &state.db,
            &state.storage,
            "Reply Document",
            Some(create_reply_reference(root_id)),
        );

        // Call the original replies handler
        let result = get_document_replies(Path(root_id), axum::extract::State(state)).await;

        // Verify success response
        assert!(result.is_ok());
        let response = result.unwrap();
        let replies = response.0;

        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].title, "Reply Document");
    }

    #[tokio::test]
    async fn test_get_document_replies_not_found() {
        let state = create_mock_app_state().await;

        // Call with non-existent document - should return empty array, not error
        let result = get_document_replies(Path(99999), axum::extract::State(state)).await;

        // The existing handler returns empty array for non-existent documents
        assert!(result.is_ok());
        let response = result.unwrap();
        let replies = response.0;
        assert_eq!(replies.len(), 0);
    }
}
