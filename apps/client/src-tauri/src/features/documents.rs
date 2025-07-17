use std::collections::HashMap;

use chrono::Utc;
use hex::FromHex;
use pod2::{
    backends::plonky2::signedpod::Signer,
    frontend::SignedPodBuilder,
    middleware::{Hash, Params},
};
use pod2_db::store::PodData;
use podnet_models::{Document, UpvoteRequest};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::Mutex;

use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentVerificationResult {
    pub publish_verified: bool,
    pub timestamp_verified: bool,
    pub upvote_count_verified: bool,
    pub verification_details: HashMap<String, String>,
    pub verification_errors: Vec<String>,
}

#[tauri::command]
pub async fn verify_document_pod(document: Document) -> Result<DocumentVerificationResult, String> {
    let mut verification_result = DocumentVerificationResult {
        publish_verified: false,
        timestamp_verified: false,
        upvote_count_verified: false,
        verification_details: HashMap::new(),
        verification_errors: Vec::new(),
    };

    // Get server public key - for now use a placeholder
    // TODO: This should be configurable or fetched from the server
    let server_public_key = "your_server_public_key_here";

    // Use the simplified Document.verify() method
    match document.verify(server_public_key) {
        Ok(()) => {
            // All verification checks passed
            verification_result.publish_verified = true;
            verification_result.timestamp_verified = true;
            verification_result.upvote_count_verified = true;

            verification_result.verification_details.insert(
                "publish_verification".to_string(),
                "Identity, document, and content hash verification passed".to_string(),
            );
            verification_result.verification_details.insert(
                "timestamp_verification".to_string(),
                "Server timestamp signature verified".to_string(),
            );
            verification_result.verification_details.insert(
                "upvote_count_verification".to_string(),
                "Upvote count cryptographic proof verified".to_string(),
            );
        }
        Err(e) => {
            verification_result
                .verification_errors
                .push(format!("Document verification failed: {}", e));
        }
    }

    Ok(verification_result)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpvoteResult {
    pub success: bool,
    pub new_upvote_count: Option<i64>,
    pub error_message: Option<String>,
    pub already_upvoted: bool,
}

#[tauri::command]
pub async fn upvote_document(
    document_id: i64,
    server_url: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<UpvoteResult, String> {
    log::info!("Upvoting document {} on server {}", document_id, server_url);

    // First, get the document to retrieve its content hash
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/documents/{}", server_url, document_id))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch document: {}", e))?;

    if !response.status().is_success() {
        return Ok(UpvoteResult {
            success: false,
            new_upvote_count: None,
            error_message: Some(format!("Failed to fetch document: {}", response.status())),
            already_upvoted: false,
        });
    }

    let document: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse document response: {}", e))?;

    let content_hash = document
        .pointer("/metadata/content_id")
        .and_then(|v| v.as_str())
        .map(Hash::from_hex)
        .ok_or("Document missing metadata.content_id field")?
        .map_err(|e| format!("Invalid content hash: {}", e))?;

    log::info!("Document content hash: {}", content_hash);

    // Get user's identity pod and private key from app state
    let app_state = state.lock().await;

    // 1. Get the app setup state to get the username and identity pod ID
    let setup_state = pod2_db::store::get_app_setup_state(&app_state.db)
        .await
        .map_err(|e| format!("Failed to get app setup state: {}", e))?;

    if !setup_state.setup_completed {
        return Err(
            "Identity setup not completed. Please complete identity setup first.".to_string(),
        );
    }

    let username = setup_state
        .username
        .ok_or("Username not found in setup state")?;

    let identity_pod_id = setup_state
        .identity_pod_id
        .ok_or("Identity pod ID not found in setup state")?;

    log::info!("Looking for identity pod with ID: {}", identity_pod_id);

    // Let's also list all pods to see what's available
    let all_pods = pod2_db::store::list_all_pods(&app_state.db)
        .await
        .map_err(|e| format!("Failed to list all pods: {}", e))?;

    log::info!("Total pods in database: {}", all_pods.len());
    for pod in &all_pods {
        log::info!(
            "Found pod - ID: {}, Type: {}, Label: {:?}",
            pod.id,
            pod.pod_type,
            pod.label
        );
    }

    // 2. Get the identity pod from the database
    let identity_pod_info = pod2_db::store::get_pod(&app_state.db, "default", &identity_pod_id)
        .await
        .map_err(|e| format!("Failed to get identity pod: {}", e))?
        .ok_or(format!(
            "Identity pod not found in database. Expected ID: {}, Available pods: {}",
            identity_pod_id,
            all_pods
                .iter()
                .map(|p| format!("{}({})", p.id, p.pod_type))
                .collect::<Vec<_>>()
                .join(", ")
        ))?;

    let identity_pod = match identity_pod_info.data {
        PodData::Signed(pod) => pod
            .try_into()
            .map_err(|e| format!("Failed to convert signed pod: {}", e))?,
        PodData::Main(_) => {
            return Err("Expected signed pod for identity, got main pod".to_string())
        }
    };

    // 3. Get the user's private key
    let private_key = pod2_db::store::get_default_private_key_raw(&app_state.db)
        .await
        .map_err(|e| format!("Failed to get private key: {}", e))?;

    // 4. Create upvote SignedPod with content_hash and timestamp
    let params = Params::default();
    let mut upvote_builder = SignedPodBuilder::new(&params);

    upvote_builder.insert("request_type", "upvote");
    upvote_builder.insert("content_hash", content_hash);
    upvote_builder.insert("timestamp", Utc::now().timestamp());

    let upvote_pod = upvote_builder
        .sign(&mut Signer(private_key))
        .map_err(|e| format!("Failed to sign upvote pod: {}", e))?;

    log::info!("✓ Upvote pod signed successfully");

    // Verify the upvote pod
    upvote_pod
        .verify()
        .map_err(|e| format!("Upvote pod verification failed: {}", e))?;

    log::info!("✓ Upvote pod verification successful");

    // 5. Generate upvote verification MainPod using prove_upvote_verification_with_solver
    let upvote_params = podnet_models::mainpod::upvote::UpvoteProofParamsSolver {
        identity_pod: &identity_pod,
        upvote_pod: &upvote_pod,
        use_mock_proofs: false, // Use real proofs for production
    };

    let upvote_main_pod =
        podnet_models::mainpod::upvote::prove_upvote_verification_with_solver(upvote_params)
            .map_err(|e| format!("Failed to generate upvote verification MainPod: {}", e))?;

    log::info!("✓ Upvote main pod created and verified");

    // Store the upvote MainPod in local database for user's records
    let upvote_pod_data = PodData::Main(upvote_main_pod.clone().into());
    let upvote_label = format!("Upvote for document {}", document_id);

    pod2_db::store::import_pod(
        &app_state.db,
        &upvote_pod_data,
        Some(&upvote_label),
        "default",
    )
    .await
    .map_err(|e| format!("Failed to store upvote pod locally: {}", e))?;

    log::info!(
        "✓ Upvote MainPod stored locally with label: {}",
        upvote_label
    );

    // 6. Submit UpvoteRequest to server
    let upvote_request = UpvoteRequest {
        username: username.clone(),
        upvote_main_pod,
    };

    log::info!("Submitting upvote to server...");
    let response = client
        .post(format!("{}/documents/{}/upvote", server_url, document_id))
        .header("Content-Type", "application/json")
        .json(&upvote_request)
        .send()
        .await
        .map_err(|e| format!("Failed to submit upvote request: {}", e))?;

    // 7. Handle response and return UpvoteResult
    if response.status().is_success() {
        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse upvote response: {}", e))?;

        let new_upvote_count = result.get("upvote_count").and_then(|v| v.as_i64());

        log::info!("✓ Successfully upvoted document!");
        if let Some(count) = new_upvote_count {
            log::info!("Document now has {} upvotes", count);
        }

        // Trigger state sync to update the UI with the new upvote pod
        // We need to release the lock first, then re-acquire it
        drop(app_state);
        let mut app_state = state.lock().await;
        if let Err(e) = app_state.trigger_state_sync().await {
            log::warn!("Failed to trigger state sync after upvote: {}", e);
        }

        Ok(UpvoteResult {
            success: true,
            new_upvote_count,
            error_message: None,
            already_upvoted: false,
        })
    } else if response.status() == StatusCode::CONFLICT {
        // Already upvoted
        Ok(UpvoteResult {
            success: false,
            new_upvote_count: None,
            error_message: Some("You have already upvoted this document".to_string()),
            already_upvoted: true,
        })
    } else {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());

        log::error!(
            "Upvote request failed with status {}: {}",
            status,
            error_text
        );

        Ok(UpvoteResult {
            success: false,
            new_upvote_count: None,
            error_message: Some(format!("Server error: {} - {}", status, error_text)),
            already_upvoted: false,
        })
    }
}
