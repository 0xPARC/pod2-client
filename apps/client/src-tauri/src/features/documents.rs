use std::collections::{HashMap, HashSet};

use chrono::Utc;
use hex::FromHex;
use pod2::{
    backends::plonky2::signedpod::Signer,
    frontend::SignedPodBuilder,
    middleware::{
        containers::{Dictionary, Set},
        hash_values, Hash, Key, Params, Value,
    },
};
use pod2_db::store::PodData;
use podnet_models::{Document, DocumentContent, DocumentFile, PublishRequest, UpvoteRequest};
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
    let identity_pod_info = pod2_db::store::get_pod(&app_state.db, "identity", &identity_pod_id)
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

    // Ensure "upvotes" folder exists
    const UPVOTES_FOLDER: &str = "upvotes";
    if !pod2_db::store::space_exists(&app_state.db, UPVOTES_FOLDER)
        .await
        .unwrap_or(false)
    {
        pod2_db::store::create_space(&app_state.db, UPVOTES_FOLDER)
            .await
            .map_err(|e| format!("Failed to create upvotes folder: {}", e))?;
        log::info!("✓ Created upvotes folder");
    }

    pod2_db::store::import_pod(
        &app_state.db,
        &upvote_pod_data,
        Some(&upvote_label),
        UPVOTES_FOLDER,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishResult {
    pub success: bool,
    pub document_id: Option<i64>,
    pub error_message: Option<String>,
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn publish_document(
    message: Option<String>,
    file: Option<DocumentFile>,
    url: Option<String>,
    tags: Vec<String>,
    authors: Vec<String>,
    reply_to: Option<i64>,
    server_url: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<PublishResult, String> {
    log::info!("Publishing document to server {}", server_url);

    // Step 1: Build DocumentContent from provided inputs
    let mut document_content = DocumentContent {
        message: None,
        file: None,
        url: None,
    };

    // Process message
    if let Some(msg) = message {
        if !msg.trim().is_empty() {
            document_content.message = Some(msg);
            log::info!("Message added to document");
        }
    }

    // Process file
    if let Some(file_data) = file {
        document_content.file = Some(file_data);
        log::info!("File added to document");
    }

    // Process URL
    if let Some(url_str) = url {
        if !url_str.trim().is_empty() {
            document_content.url = Some(url_str.clone());
            log::info!("URL added to document: {}", url_str);
        }
    }

    // Validate that at least one content type is provided
    document_content
        .validate()
        .map_err(|e| format!("Content validation failed: {}", e))?;

    // Step 2: Get user's identity pod and private key from app state
    let app_state = state.lock().await;

    // Get the app setup state to get the username and identity pod ID
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

    // Get the identity pod from the database
    let identity_pod_info = pod2_db::store::get_pod(&app_state.db, "identity", &identity_pod_id)
        .await
        .map_err(|e| format!("Failed to get identity pod: {}", e))?
        .ok_or(format!(
            "Identity pod not found in database with ID: {}",
            identity_pod_id
        ))?;

    let identity_pod: pod2::frontend::SignedPod = match identity_pod_info.data {
        PodData::Signed(pod) => pod
            .try_into()
            .map_err(|e| format!("Failed to convert signed pod: {}", e))?,
        PodData::Main(_) => {
            return Err("Expected signed pod for identity, got main pod".to_string())
        }
    };

    // Verify the identity pod
    identity_pod
        .verify()
        .map_err(|e| format!("Identity pod verification failed: {}", e))?;
    log::info!("✓ Identity pod verification successful");

    // Get the user's private key
    let private_key = pod2_db::store::get_default_private_key_raw(&app_state.db)
        .await
        .map_err(|e| format!("Failed to get private key: {}", e))?;

    // Step 3: Process tags and authors
    let document_tags: HashSet<String> = tags
        .into_iter()
        .map(|tag| tag.trim().to_string())
        .filter(|tag| !tag.is_empty())
        .collect();

    let document_authors: HashSet<String> = if authors.is_empty() {
        // Default to uploader if no authors provided
        let mut default_authors = HashSet::new();
        default_authors.insert(username.clone());
        default_authors
    } else {
        authors
            .into_iter()
            .map(|author| author.trim().to_string())
            .filter(|author| !author.is_empty())
            .collect()
    };

    // Step 4: Compute content hash from the entire DocumentContent structure
    let content_json = serde_json::to_string(&document_content)
        .map_err(|e| format!("Failed to serialize document content: {}", e))?;
    let content_hash = hash_values(&[Value::from(content_json)]);

    log::info!("Content hash: {}", content_hash);
    log::info!("Tags: {:?}", document_tags);
    log::info!("Authors: {:?}", document_authors);

    // Step 5: Create document pod
    let params = Params::default();

    let tag_set = Set::new(
        5,
        document_tags
            .iter()
            .map(|v| Value::from(v.clone()))
            .collect(),
    )
    .map_err(|e| format!("Failed to create tag set: {}", e))?;

    let authors_set = Set::new(
        5,
        document_authors
            .iter()
            .map(|author| Value::from(author.as_str()))
            .collect(),
    )
    .map_err(|e| format!("Failed to create authors set: {}", e))?;

    let data_dict = Dictionary::new(
        6,
        HashMap::from([
            (Key::from("authors"), Value::from(authors_set)),
            (Key::from("content_hash"), Value::from(content_hash)),
            (Key::from("tags"), Value::from(tag_set)),
            (Key::from("post_id"), Value::from(-1i64)), // Will be assigned by server
            (Key::from("reply_to"), Value::from(reply_to.unwrap_or(-1))),
        ]),
    )
    .map_err(|e| format!("Failed to create data dictionary: {}", e))?;

    let mut document_builder = SignedPodBuilder::new(&params);
    document_builder.insert("request_type", "publish");
    document_builder.insert("data", data_dict.clone());

    let document_pod = document_builder
        .sign(&mut Signer(private_key))
        .map_err(|e| format!("Failed to sign document pod: {}", e))?;

    log::info!("✓ Document pod signed successfully");

    // Verify the document pod
    document_pod
        .verify()
        .map_err(|e| format!("Document pod verification failed: {}", e))?;
    log::info!("✓ Document pod verification successful");

    // Step 6: Create main pod that proves both identity and document verification
    let publish_params = podnet_models::mainpod::publish::PublishProofParams {
        identity_pod: &identity_pod,
        document_pod: &document_pod,
        use_mock_proofs: false, // Use real proofs for production
    };

    let publish_main_pod =
        podnet_models::mainpod::publish::prove_publish_verification_with_solver(publish_params)
            .map_err(|e| format!("Failed to generate publish verification MainPod: {}", e))?;

    // Verify the main pod
    podnet_models::mainpod::publish::verify_publish_verification_with_solver(
        &publish_main_pod,
        &username,
        &data_dict,
        identity_pod.get(pod2::middleware::KEY_SIGNER).unwrap(),
    )
    .map_err(|e| format!("Failed to verify publish verification MainPod: {}", e))?;

    log::info!("✓ Publish main pod created and verified");

    // Step 7: Store the publish MainPod in local database for user's records
    let publish_pod_data = PodData::Main(publish_main_pod.clone().into());
    let publish_label = format!("Publish document with content hash {}", content_hash);

    // Ensure "published" folder exists
    const PUBLISHED_FOLDER: &str = "published";
    if !pod2_db::store::space_exists(&app_state.db, PUBLISHED_FOLDER)
        .await
        .unwrap_or(false)
    {
        pod2_db::store::create_space(&app_state.db, PUBLISHED_FOLDER)
            .await
            .map_err(|e| format!("Failed to create published folder: {}", e))?;
        log::info!("✓ Created published folder");
    }

    pod2_db::store::import_pod(
        &app_state.db,
        &publish_pod_data,
        Some(&publish_label),
        PUBLISHED_FOLDER,
    )
    .await
    .map_err(|e| format!("Failed to store publish pod locally: {}", e))?;

    log::info!(
        "✓ Publish MainPod stored locally with label: {}",
        publish_label
    );

    // Step 8: Create the publish request
    let publish_request = PublishRequest {
        content: document_content,
        tags: document_tags,
        authors: document_authors,
        reply_to,
        post_id: None, // Will be assigned by server
        username: username.clone(),
        main_pod: publish_main_pod,
    };

    log::info!("Sending publish request to server...");

    // Step 9: Submit PublishRequest to server
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/publish", server_url))
        .header("Content-Type", "application/json")
        .json(&publish_request)
        .send()
        .await
        .map_err(|e| format!("Failed to submit publish request: {}", e))?;

    // Step 10: Handle response and return PublishResult
    if response.status().is_success() {
        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse publish response: {}", e))?;

        let document_id = result.pointer("/metadata/id").and_then(|v| v.as_i64());

        log::info!("✓ Successfully published document!");
        if let Some(id) = document_id {
            log::info!("Document assigned ID: {}", id);
        }

        // Trigger state sync to update the UI with the new publish pod
        drop(app_state);
        let mut app_state = state.lock().await;
        if let Err(e) = app_state.trigger_state_sync().await {
            log::warn!("Failed to trigger state sync after publish: {}", e);
        }

        Ok(PublishResult {
            success: true,
            document_id,
            error_message: None,
        })
    } else {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());

        log::error!(
            "Publish request failed with status {}: {}",
            status,
            error_text
        );

        Ok(PublishResult {
            success: false,
            document_id: None,
            error_message: Some(format!("Server error: {} - {}", status, error_text)),
        })
    }
}
