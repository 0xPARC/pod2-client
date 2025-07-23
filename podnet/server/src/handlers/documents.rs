use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use pod2::middleware::{
    Key, Value,
    containers::{Dictionary, Set},
};
use podnet_models::{
    Document, DocumentMetadata, PublishRequest,
    mainpod::publish::verify_publish_verification_with_solver,
};

pub async fn get_documents(
    State(state): State<Arc<crate::AppState>>,
) -> Result<Json<Vec<DocumentMetadata>>, StatusCode> {
    let documents_metadata = state
        .db
        .get_all_documents_metadata()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(documents_metadata))
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
    log::info!("Starting document publish with main pod verification");

    // Validate the document content
    payload.content.validate().map_err(|e| {
        log::error!("Document content validation failed: {e}");
        StatusCode::BAD_REQUEST
    })?;
    log::info!("✓ Document content validated");

    // Validate the title
    if payload.title.trim().is_empty() {
        log::error!("Document title cannot be empty");
        return Err(StatusCode::BAD_REQUEST);
    }
    log::info!("✓ Document title validated");

    let (_vd_set, _prover) = state.pod_config.get_prover_setup()?;

    // Verify main pod proof
    log::info!("Verifying main pod proof");
    payload.main_pod.pod.verify().map_err(|e| {
        log::error!("Failed to verify main pod: {e}");
        StatusCode::UNAUTHORIZED
    })?;
    log::info!("✓ Main pod proof verified");

    // Store the content first to get its hash for verification
    log::info!("Storing content in content-addressed storage");
    let stored_content_hash = state
        .storage
        .store_document_content(&payload.content)
        .map_err(|e| {
            log::error!("Failed to store content: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    log::info!("Content stored successfully with hash: {stored_content_hash}");

    // Create the expected data structure for verification using request data
    log::info!("Creating expected data structure for solver verification");
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
        log::error!("Failed to create tags set: {e:?}");
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
        log::error!("Failed to create authors set: {e:?}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    data_map.insert(Key::from("authors"), Value::from(authors_set));

    // Add reply_to (convert ReplyReference to dictionary or use -1 if None)
    if let Some(ref reply_ref) = payload.reply_to {
        let mut reply_map = HashMap::new();
        reply_map.insert(Key::from("post_id"), Value::from(reply_ref.post_id));
        reply_map.insert(Key::from("document_id"), Value::from(reply_ref.document_id));
        let reply_dict = Dictionary::new(2, reply_map).map_err(|e| {
            log::error!("Failed to create reply_to dictionary: {e:?}");
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
        log::error!("Failed to create expected data dictionary: {e:?}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // We need to first verify with all registered identity servers, since we don't know which one was used
    log::info!("Getting all registered identity servers for verification");
    let identity_servers = state.db.get_all_identity_servers().map_err(|e| {
        log::error!("Database error retrieving identity servers: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if identity_servers.is_empty() {
        log::error!("No identity servers registered");
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Try verification with each registered identity server until one succeeds
    let mut verification_succeeded = false;
    let mut identity_server_pk = None;

    for identity_server in &identity_servers {
        // Parse the identity server public key from database
        let server_pk: pod2::backends::plonky2::primitives::ec::curve::Point =
            serde_json::from_str(&identity_server.public_key).map_err(|e| {
                log::error!("Failed to parse identity server public key: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let server_pk_value = Value::from(server_pk);

        // Try verification with this identity server
        log::info!(
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
                log::info!(
                    "✓ Solver verification succeeded with identity server: {}",
                    identity_server.server_id
                );
                verification_succeeded = true;
                identity_server_pk = Some(server_pk);
                break;
            }
            Err(_) => {
                log::debug!(
                    "Verification failed with identity server: {}",
                    identity_server.server_id
                );
                continue;
            }
        }
    }

    if !verification_succeeded {
        log::error!("Solver-based verification failed with all registered identity servers");
        return Err(StatusCode::BAD_REQUEST);
    }

    let _identity_server_pk = identity_server_pk.unwrap();

    log::info!(
        "✓ Solver verification passed: username={}, content_hash={stored_content_hash}",
        payload.username
    );

    // Use the data from the request for further processing
    let uploader_username = &payload.username;
    let post_id = payload.post_id.unwrap_or(-1);
    let content_hash = stored_content_hash;

    // Identity server verification was already done above during solver verification

    // Determine post_id: either create new post or use existing
    log::info!("Determining post ID");
    let final_post_id = if post_id != -1 {
        log::info!("Using existing post ID: {post_id}");
        // Verify the post exists
        state
            .db
            .get_post(post_id)
            .map_err(|e| {
                log::error!("Database error checking post {post_id}: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .ok_or_else(|| {
                log::error!("Post {post_id} not found");
                StatusCode::NOT_FOUND
            })?;
        log::info!("Post {post_id} exists");
        post_id
    } else {
        log::info!("Creating new post");
        let id = state.db.create_post().map_err(|e| {
            log::error!("Failed to create new post: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        log::info!("New post created with ID: {id}");
        id
    };

    // Validate reply_to if provided
    if let Some(ref reply_ref) = payload.reply_to {
        log::info!(
            "Validating reply_to document ID: {} in post: {}",
            reply_ref.document_id,
            reply_ref.post_id
        );

        // Verify the document being replied to exists
        let target_doc = state
            .db
            .get_document_metadata(reply_ref.document_id)
            .map_err(|e| {
                log::error!(
                    "Database error checking reply_to document {}: {e}",
                    reply_ref.document_id
                );
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .ok_or_else(|| {
                log::error!("Reply_to document {} not found", reply_ref.document_id);
                StatusCode::NOT_FOUND
            })?;

        // Verify the post_id matches
        if target_doc.post_id != reply_ref.post_id {
            log::error!(
                "Reply_to post_id {} doesn't match document's actual post_id {}",
                reply_ref.post_id,
                target_doc.post_id
            );
            return Err(StatusCode::BAD_REQUEST);
        }

        log::info!(
            "Reply_to reference validated: document {} in post {}",
            reply_ref.document_id,
            reply_ref.post_id
        );
    }

    // Create document with timestamp pod in a single transaction
    log::info!("Creating document for post {final_post_id}");
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
            log::error!("Failed to create document: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    log::info!("Document created with ID: {:?}", document.metadata.id);

    // Spawn background task to generate base case upvote count pod
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
                log::error!(
                    "Failed to generate base case upvote count pod for document {document_id}: {e}"
                );
            }
        });
    }

    log::info!("Document publish completed successfully using main pod verification");
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
