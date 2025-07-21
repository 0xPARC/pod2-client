use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use pod_utils::ValueExt;
use pod2::backends::plonky2::{
    primitives::ec::schnorr::SecretKey,
    signedpod::Signer,
};
use pod2::frontend::{MainPod, SignedPodBuilder};
use pod2::middleware::{Hash, Value};
use podnet_models::{
    UpvoteRequest, get_upvote_verification_predicate,
    mainpod::upvote::{verify_upvote_verification_with_solver, prove_upvote_count_base_with_solver, UpvoteCountBaseParams, prove_upvote_count_inductive_with_solver, UpvoteCountInductiveParams},
};
use std::sync::Arc;

use crate::pod::get_server_secret_key;

pub async fn upvote_document(
    Path(document_id): Path<i64>,
    State(state): State<Arc<crate::AppState>>,
    Json(payload): Json<UpvoteRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {

    log::info!("Processing upvote for document {document_id} with main pod verification");

    let (_vd_set, _prover) = state.pod_config.get_prover_setup()?;

    // Verify main pod proof
    log::info!("Verifying upvote main pod proof");
    payload.upvote_main_pod.pod.verify().map_err(|e| {
        log::error!("Failed to verify upvote main pod: {e}");
        StatusCode::UNAUTHORIZED
    })?;
    log::info!("✓ Upvote main pod proof verified");

    // Get the document first to get its content hash for verification
    log::info!("Getting document for content hash verification");
    let document = state
        .db
        .get_document_metadata(document_id)
        .map_err(|e| {
            log::error!("Database error retrieving document {document_id}: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            log::error!("Document {document_id} not found");
            StatusCode::NOT_FOUND
        })?;

    // We need to verify with all registered identity servers, since we don't know which one was used
    log::info!("Getting all registered identity servers for verification");
    let identity_servers = state
        .db
        .get_all_identity_servers()
        .map_err(|e| {
            log::error!("Database error retrieving identity servers: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if identity_servers.is_empty() {
        log::error!("No identity servers registered");
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Try verification with each registered identity server until one succeeds
    let mut verification_succeeded = false;

    for identity_server in &identity_servers {
        // Parse the identity server public key from database
        let server_pk: pod2::backends::plonky2::primitives::ec::curve::Point = 
            serde_json::from_str(&identity_server.public_key).map_err(|e| {
                log::error!("Failed to parse identity server public key: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let server_pk_value = Value::from(server_pk);

        // Try verification with this identity server using username from request
        log::info!("Trying upvote verification with identity server: {}", identity_server.server_id);
        match verify_upvote_verification_with_solver(
            &payload.upvote_main_pod,
            &payload.username,
            &document.content_id,
            &server_pk_value,
        ) {
            Ok(_) => {
                log::info!("✓ Solver verification succeeded with identity server: {}", identity_server.server_id);
                verification_succeeded = true;
                break;
            }
            Err(_) => {
                log::debug!("Verification failed with identity server: {}", identity_server.server_id);
                continue;
            }
        }
    }

    if !verification_succeeded {
        log::error!("Solver-based verification failed with all registered identity servers");
        return Err(StatusCode::BAD_REQUEST);
    }

    log::info!(
        "✓ Solver verification passed: username={}, content_hash={}",
        payload.username, document.content_id
    );

    // Content hash verification was already done during solver verification

    // Check if user has already upvoted this document (by username)
    let already_upvoted = state
        .db
        .user_has_upvoted(document_id, &payload.username)
        .map_err(|e| {
            log::error!("Database error checking existing upvote: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if already_upvoted {
        log::warn!(
            "User {} has already upvoted document {document_id}",
            payload.username
        );
        return Err(StatusCode::CONFLICT);
    }

    // Store the upvote with the main pod (no user public key needed)
    let upvote_main_pod_json = serde_json::to_string(&payload.upvote_main_pod).map_err(|e| {
        log::error!("Failed to serialize upvote main pod: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let upvote_id = state
        .db
        .create_upvote(document_id, &payload.username, &upvote_main_pod_json)
        .map_err(|e| {
            log::error!("Failed to store upvote: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    log::info!("✓ Upvote stored with ID: {upvote_id}");

    // Get updated upvote count
    let upvote_count = state.db.get_upvote_count(document_id).map_err(|e| {
        log::error!("Failed to get upvote count: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    log::info!("Document {document_id} now has {upvote_count} upvotes");

    // Spawn background task to generate inductive upvote count pod
    let state_clone = state.clone();
    let doc_id = document_id;
    let hash = document.content_id;
    let current_count = upvote_count;

    tokio::spawn(async move {
        if let Err(e) = generate_inductive_upvote_pod(
            state_clone,
            doc_id,
            &hash,
            current_count,
            &payload.upvote_main_pod,
        )
        .await
        {
            log::error!(
                "Failed to generate inductive upvote count pod for document {doc_id}: {e}"
            );
        }
    });

    Ok(Json(serde_json::json!({
        "success": true,
        "upvote_id": upvote_id,
        "document_id": document_id,
        "upvote_count": upvote_count
    })))
}

pub async fn generate_base_case_upvote_pod(
    state: Arc<crate::AppState>,
    document_id: i64,
    content_hash: &Hash,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing::info!(
        "Generating base case upvote count pod for document {} using solver",
        document_id
    );

    // Use the solver-based approach for base case upvote count proof
    let params = UpvoteCountBaseParams {
        content_hash,
        use_mock_proofs: state.pod_config.is_mock(),
    };

    let main_pod = prove_upvote_count_base_with_solver(params)
        .map_err(|e| format!("Failed to generate base case upvote count pod: {e}"))?;

    main_pod.pod.verify()
        .map_err(|e| format!("Failed to verify base case upvote count pod: {e}"))?;

    log::info!(
        "✓ Successfully proved upvote_count(0) for document {document_id} using solver"
    );

    // Store the pod in the database
    let pod_json = serde_json::to_string(&main_pod)
        .map_err(|e| format!("Failed to serialize main pod: {e}"))?;

    state
        .db
        .update_upvote_count_pod(document_id, &pod_json)
        .map_err(|e| format!("Failed to store upvote count pod: {e}"))?;

    log::info!(
        "✓ Stored base case upvote count pod for document {document_id}"
    );

    Ok(())
}

async fn generate_inductive_upvote_pod(
    state: Arc<crate::AppState>,
    document_id: i64,
    content_hash: &Hash,
    current_count: i64,
    upvote_verification_pod: &MainPod,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    log::info!(
        "Generating inductive upvote count pod for document {document_id} (count={current_count}) using solver"
    );

    // Get the previous upvote count pod from database (for recursive proof)
    let previous_pod_json = state
        .db
        .get_upvote_count_pod(document_id)
        .map_err(|e| format!("Failed to get previous upvote count pod: {e}"))?;

    let previous_pod = match previous_pod_json {
        Some(json) => serde_json::from_str::<pod2::frontend::MainPod>(&json)
            .map_err(|e| format!("Failed to parse previous main pod: {e}"))?,
        None => {
            log::warn!(
                "No previous upvote count pod found for document {document_id}, generating base case first"
            );
            // If no previous pod exists, generate base case first
            generate_base_case_upvote_pod(state.clone(), document_id, content_hash).await?;

            // Then get the newly created base case pod
            let base_pod_json = state
                .db
                .get_upvote_count_pod(document_id)
                .map_err(|e| format!("Failed to get base case pod after generation: {e}"))?
                .ok_or("Base case pod not found after generation")?;

            serde_json::from_str::<pod2::frontend::MainPod>(&base_pod_json)
                .map_err(|e| format!("Failed to parse base case main pod: {e}"))?
        }
    };

    // Use the solver-based approach for inductive case upvote count proof
    let params = UpvoteCountInductiveParams {
        content_hash,
        previous_count: current_count - 1,
        previous_count_pod: &previous_pod,
        upvote_verification_pod,
        use_mock_proofs: state.pod_config.is_mock(),
    };

    let main_pod = prove_upvote_count_inductive_with_solver(params)
        .map_err(|e| format!("Failed to generate inductive upvote count pod: {e}"))?;

    main_pod.pod.verify()
        .map_err(|e| format!("Failed to verify inductive upvote count pod: {e}"))?;

    log::info!(
        "✓ Successfully proved upvote_count({current_count}) for document {document_id} using solver"
    );

    // Store the pod in the database
    let pod_json = serde_json::to_string(&main_pod)
        .map_err(|e| format!("Failed to serialize main pod: {e}"))?;

    state
        .db
        .update_upvote_count_pod(document_id, &pod_json)
        .map_err(|e| format!("Failed to store upvote count pod: {e}"))?;

    log::info!(
        "✓ Stored inductive upvote count pod for document {document_id} (count={current_count})"
    );

    Ok(())
}
