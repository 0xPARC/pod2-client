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
use podnet_models::{
    DeleteRequest, Document, DocumentContent, DocumentFile, PublishRequest, ReplyReference,
    UpvoteRequest,
};
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
                .push(format!("Document verification failed: {e}"));
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
    log::info!("Upvoting document {document_id} on server {server_url}");

    // First, get the document to retrieve its content hash
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{server_url}/documents/{document_id}"))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch document: {e}"))?;

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
        .map_err(|e| format!("Failed to parse document response: {e}"))?;

    let content_hash = document
        .pointer("/metadata/content_id")
        .and_then(|v| v.as_str())
        .map(Hash::from_hex)
        .ok_or("Document missing metadata.content_id field")?
        .map_err(|e| format!("Invalid content hash: {e}"))?;

    log::info!("Document content hash: {content_hash}");

    // Get user's identity pod and private key from app state
    let app_state = state.lock().await;

    // 1. Get the app setup state to get the username and identity pod ID
    let setup_state = pod2_db::store::get_app_setup_state(&app_state.db)
        .await
        .map_err(|e| format!("Failed to get app setup state: {e}"))?;

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

    log::info!("Looking for identity pod with ID: {identity_pod_id}");

    // Let's also list all pods to see what's available
    let all_pods = pod2_db::store::list_all_pods(&app_state.db)
        .await
        .map_err(|e| format!("Failed to list all pods: {e}"))?;

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
        .map_err(|e| format!("Failed to get identity pod: {e}"))?
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
        PodData::Signed(pod) => (*pod)
            .try_into()
            .map_err(|e| format!("Failed to convert signed pod: {e}"))?,
        PodData::Main(_) => {
            return Err("Expected signed pod for identity, got main pod".to_string())
        }
    };

    // 3. Get the user's private key
    let private_key = pod2_db::store::get_default_private_key_raw(&app_state.db)
        .await
        .map_err(|e| format!("Failed to get private key: {e}"))?;

    // 4. Create upvote SignedPod with content_hash and timestamp
    let params = Params::default();
    let mut upvote_builder = SignedPodBuilder::new(&params);

    upvote_builder.insert("request_type", "upvote");
    upvote_builder.insert("content_hash", content_hash);
    upvote_builder.insert("timestamp", Utc::now().timestamp());

    let upvote_pod = upvote_builder
        .sign(&Signer(private_key))
        .map_err(|e| format!("Failed to sign upvote pod: {e}"))?;

    log::info!("✓ Upvote pod signed successfully");

    // Verify the upvote pod
    upvote_pod
        .verify()
        .map_err(|e| format!("Upvote pod verification failed: {e}"))?;

    log::info!("✓ Upvote pod verification successful");

    // 5. Generate upvote verification MainPod using prove_upvote_verification_with_solver
    let upvote_params = podnet_models::mainpod::upvote::UpvoteProofParamsSolver {
        identity_pod: &identity_pod,
        upvote_pod: &upvote_pod,
        use_mock_proofs: false, // Use real proofs for production
    };

    let upvote_main_pod =
        podnet_models::mainpod::upvote::prove_upvote_verification_with_solver(upvote_params)
            .map_err(|e| format!("Failed to generate upvote verification MainPod: {e}"))?;

    log::info!("✓ Upvote main pod created and verified");

    // Store the upvote MainPod in local database for user's records
    let upvote_pod_data = PodData::Main(Box::new(upvote_main_pod.clone().into()));
    let upvote_label = format!("Upvote for document {document_id}");

    // Ensure "upvotes" folder exists
    const UPVOTES_FOLDER: &str = "upvotes";
    if !pod2_db::store::space_exists(&app_state.db, UPVOTES_FOLDER)
        .await
        .unwrap_or(false)
    {
        pod2_db::store::create_space(&app_state.db, UPVOTES_FOLDER)
            .await
            .map_err(|e| format!("Failed to create upvotes folder: {e}"))?;
        log::info!("✓ Created upvotes folder");
    }

    pod2_db::store::import_pod(
        &app_state.db,
        &upvote_pod_data,
        Some(&upvote_label),
        UPVOTES_FOLDER,
    )
    .await
    .map_err(|e| format!("Failed to store upvote pod locally: {e}"))?;

    log::info!("✓ Upvote MainPod stored locally with label: {upvote_label}");

    // 6. Submit UpvoteRequest to server
    let upvote_request = UpvoteRequest {
        username: username.clone(),
        upvote_main_pod,
    };

    log::info!("Submitting upvote to server...");
    let response = client
        .post(format!("{server_url}/documents/{document_id}/upvote"))
        .header("Content-Type", "application/json")
        .json(&upvote_request)
        .send()
        .await
        .map_err(|e| format!("Failed to submit upvote request: {e}"))?;

    // 7. Handle response and return UpvoteResult
    if response.status().is_success() {
        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse upvote response: {e}"))?;

        let new_upvote_count = result.get("upvote_count").and_then(|v| v.as_i64());

        log::info!("✓ Successfully upvoted document!");
        if let Some(count) = new_upvote_count {
            log::info!("Document now has {count} upvotes");
        }

        // Trigger state sync to update the UI with the new upvote pod
        // We need to release the lock first, then re-acquire it
        drop(app_state);
        let mut app_state = state.lock().await;
        if let Err(e) = app_state.trigger_state_sync().await {
            log::warn!("Failed to trigger state sync after upvote: {e}");
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

        log::error!("Upvote request failed with status {status}: {error_text}");

        Ok(UpvoteResult {
            success: false,
            new_upvote_count: None,
            error_message: Some(format!("Server error: {status} - {error_text}")),
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
    title: String,
    message: Option<String>,
    file: Option<DocumentFile>,
    url: Option<String>,
    tags: Vec<String>,
    authors: Vec<String>,
    reply_to: Option<ReplyReference>,
    server_url: String,
    draft_id: Option<String>, // UUID of draft to delete after successful publish
    post_id: Option<i64>,     // Optional post ID for creating revisions (editing documents)
    state: State<'_, Mutex<AppState>>,
) -> Result<PublishResult, String> {
    log::info!("Publishing document to server {server_url}");
    log::info!("Post ID for revision: {post_id:?}");
    if let Some(ref reply_ref) = reply_to {
        log::info!(
            "Replying to post {} document {}",
            reply_ref.post_id,
            reply_ref.document_id
        );
    }

    // Validate title
    if title.trim().is_empty() {
        return Err("Title cannot be empty".to_string());
    }
    if title.len() > 200 {
        return Err("Title cannot exceed 200 characters".to_string());
    }

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
            log::info!("URL added to document: {url_str}");
        }
    }

    // Validate that at least one content type is provided
    document_content
        .validate()
        .map_err(|e| format!("Content validation failed: {e}"))?;

    // Step 2: Get user's identity pod and private key from app state
    let app_state = state.lock().await;

    // Get the app setup state to get the username and identity pod ID
    let setup_state = pod2_db::store::get_app_setup_state(&app_state.db)
        .await
        .map_err(|e| format!("Failed to get app setup state: {e}"))?;

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

    log::info!("Looking for identity pod with ID: {identity_pod_id}");

    // Get the identity pod from the database
    let identity_pod_info = pod2_db::store::get_pod(&app_state.db, "identity", &identity_pod_id)
        .await
        .map_err(|e| format!("Failed to get identity pod: {e}"))?
        .ok_or(format!(
            "Identity pod not found in database with ID: {identity_pod_id}"
        ))?;

    let identity_pod: pod2::frontend::SignedPod = match identity_pod_info.data {
        PodData::Signed(pod) => (*pod)
            .try_into()
            .map_err(|e| format!("Failed to convert signed pod: {e}"))?,
        PodData::Main(_) => {
            return Err("Expected signed pod for identity, got main pod".to_string())
        }
    };

    // Verify the identity pod
    identity_pod
        .verify()
        .map_err(|e| format!("Identity pod verification failed: {e}"))?;
    log::info!("✓ Identity pod verification successful");

    // Get the user's private key
    let private_key = pod2_db::store::get_default_private_key_raw(&app_state.db)
        .await
        .map_err(|e| format!("Failed to get private key: {e}"))?;

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
        .map_err(|e| format!("Failed to serialize document content: {e}"))?;
    let content_hash = hash_values(&[Value::from(content_json)]);

    log::info!("Content hash: {content_hash}");
    log::info!("Tags: {document_tags:?}");
    log::info!("Authors: {document_authors:?}");

    // Step 5: Create document pod
    let params = Params::default();

    let tag_set = Set::new(
        5,
        document_tags
            .iter()
            .map(|v| Value::from(v.clone()))
            .collect(),
    )
    .map_err(|e| format!("Failed to create tag set: {e}"))?;

    let authors_set = Set::new(
        5,
        document_authors
            .iter()
            .map(|author| Value::from(author.as_str()))
            .collect(),
    )
    .map_err(|e| format!("Failed to create authors set: {e}"))?;

    // Create reply_to value
    let reply_to_value = if let Some(ref reply_ref) = reply_to {
        let mut reply_map = HashMap::new();
        reply_map.insert(Key::from("post_id"), Value::from(reply_ref.post_id));
        reply_map.insert(Key::from("document_id"), Value::from(reply_ref.document_id));
        let reply_dict = Dictionary::new(2, reply_map)
            .map_err(|e| format!("Failed to create reply_to dictionary: {e}"))?;
        Value::from(reply_dict)
    } else {
        Value::from(-1i64)
    };

    let data_dict = Dictionary::new(
        6,
        HashMap::from([
            (Key::from("authors"), Value::from(authors_set)),
            (Key::from("content_hash"), Value::from(content_hash)),
            (Key::from("tags"), Value::from(tag_set)),
            (Key::from("post_id"), Value::from(post_id.unwrap_or(-1i64))), // Use provided post_id for edits, or -1 for new documents
            (Key::from("reply_to"), reply_to_value),
        ]),
    )
    .map_err(|e| format!("Failed to create data dictionary: {e}"))?;

    let mut document_builder = SignedPodBuilder::new(&params);
    document_builder.insert("request_type", "publish");
    document_builder.insert("data", data_dict.clone());

    let document_pod = document_builder
        .sign(&Signer(private_key))
        .map_err(|e| format!("Failed to sign document pod: {e}"))?;

    log::info!("✓ Document pod signed successfully");

    // Verify the document pod
    document_pod
        .verify()
        .map_err(|e| format!("Document pod verification failed: {e}"))?;
    log::info!("✓ Document pod verification successful");

    // Step 6: Create main pod that proves both identity and document verification
    let publish_params = podnet_models::mainpod::publish::PublishProofParams {
        identity_pod: &identity_pod,
        document_pod: &document_pod,
        use_mock_proofs: false, // Use real proofs for production
    };

    let publish_main_pod =
        podnet_models::mainpod::publish::prove_publish_verification_with_solver(publish_params)
            .map_err(|e| format!("Failed to generate publish verification MainPod: {e}"))?;

    // Verify the main pod
    podnet_models::mainpod::publish::verify_publish_verification_with_solver(
        &publish_main_pod,
        &username,
        &data_dict,
        identity_pod.get(pod2::middleware::KEY_SIGNER).unwrap(),
    )
    .map_err(|e| format!("Failed to verify publish verification MainPod: {e}"))?;

    log::info!("✓ Publish main pod created and verified");

    // Step 7: Store the publish MainPod in local database for user's records
    let publish_pod_data = PodData::Main(Box::new(publish_main_pod.clone().into()));
    let publish_label = format!("Publish document with content hash {content_hash}");

    // Ensure "published" folder exists
    const PUBLISHED_FOLDER: &str = "published";
    if !pod2_db::store::space_exists(&app_state.db, PUBLISHED_FOLDER)
        .await
        .unwrap_or(false)
    {
        pod2_db::store::create_space(&app_state.db, PUBLISHED_FOLDER)
            .await
            .map_err(|e| format!("Failed to create published folder: {e}"))?;
        log::info!("✓ Created published folder");
    }

    // Check if POD already exists before storing
    let pod_id = publish_pod_data.id();
    match pod2_db::store::get_pod(&app_state.db, &pod_id, PUBLISHED_FOLDER).await {
        Ok(_) => {
            log::info!("✓ Publish MainPod already exists locally with ID: {pod_id}");
        }
        Err(_) => {
            // POD doesn't exist, store it
            pod2_db::store::import_pod(
                &app_state.db,
                &publish_pod_data,
                Some(&publish_label),
                PUBLISHED_FOLDER,
            )
            .await
            .map_err(|e| format!("Failed to store publish pod locally: {e}"))?;
            log::info!("✓ Publish MainPod stored locally with label: {publish_label}");
        }
    }

    // Step 8: Create the publish request
    let publish_request = PublishRequest {
        title: title.trim().to_string(),
        content: document_content,
        tags: document_tags,
        authors: document_authors,
        reply_to,
        post_id, // Use provided post_id for revisions, or None for new documents
        username: username.clone(),
        main_pod: publish_main_pod,
    };

    log::info!("Sending publish request to server...");
    log::info!(
        "PublishRequest post_id field: {:?}",
        publish_request.post_id
    );

    // Step 9: Submit PublishRequest to server
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{server_url}/publish"))
        .header("Content-Type", "application/json")
        .json(&publish_request)
        .send()
        .await
        .map_err(|e| format!("Failed to submit publish request: {e}"))?;

    // Step 10: Handle response and return PublishResult
    if response.status().is_success() {
        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse publish response: {e}"))?;

        let document_id = result.pointer("/metadata/id").and_then(|v| v.as_i64());

        log::info!("✓ Successfully published document!");
        if let Some(id) = document_id {
            log::info!("Document assigned ID: {id}");
        }

        // If a draft_id was provided, delete the draft after successful publishing
        if let Some(ref draft_id) = draft_id {
            if let Err(e) = pod2_db::store::delete_draft(&app_state.db, draft_id).await {
                log::warn!("Failed to delete draft after successful publish: {e}");
            } else {
                log::info!("Draft {draft_id} deleted after successful publish");
            }
        }

        // Trigger state sync to update the UI with the new publish pod
        drop(app_state);
        let mut app_state = state.lock().await;
        if let Err(e) = app_state.trigger_state_sync().await {
            log::warn!("Failed to trigger state sync after publish: {e}");
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

        log::error!("Publish request failed with status {status}: {error_text}");

        Ok(PublishResult {
            success: false,
            document_id: None,
            error_message: Some(format!("Server error: {status} - {error_text}")),
        })
    }
}

// --- Draft Management Commands ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftUpdateRequest {
    pub title: String,
    pub content_type: String,
    pub message: Option<String>,
    pub file_name: Option<String>,
    pub file_content: Option<Vec<u8>>,
    pub file_mime_type: Option<String>,
    pub url: Option<String>,
    pub tags: Vec<String>,
    pub authors: Vec<String>,
    pub reply_to: Option<String>,
}

#[tauri::command]
pub async fn create_draft(
    request: DraftUpdateRequest,
    state: State<'_, Mutex<AppState>>,
) -> Result<String, String> {
    let app_state = state.lock().await;

    let create_request = pod2_db::store::CreateDraftRequest {
        title: request.title,
        content_type: request.content_type,
        message: request.message,
        file_name: request.file_name,
        file_content: request.file_content,
        file_mime_type: request.file_mime_type,
        url: request.url,
        tags: request.tags,
        authors: request.authors,
        reply_to: request.reply_to,
    };

    pod2_db::store::create_draft(&app_state.db, create_request)
        .await
        .map_err(|e| format!("Failed to create draft: {e}"))
}

#[tauri::command]
pub async fn update_draft(
    draft_id: String,
    request: DraftUpdateRequest,
    state: State<'_, Mutex<AppState>>,
) -> Result<bool, String> {
    let app_state = state.lock().await;

    let update_request = pod2_db::store::UpdateDraftRequest {
        title: request.title,
        content_type: request.content_type,
        message: request.message,
        file_name: request.file_name,
        file_content: request.file_content,
        file_mime_type: request.file_mime_type,
        url: request.url,
        tags: request.tags,
        authors: request.authors,
        reply_to: request.reply_to,
    };

    pod2_db::store::update_draft(&app_state.db, &draft_id, update_request)
        .await
        .map_err(|e| format!("Failed to update draft: {e}"))
}

#[tauri::command]
pub async fn list_drafts(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<pod2_db::store::DraftInfo>, String> {
    let app_state = state.lock().await;

    pod2_db::store::list_drafts(&app_state.db)
        .await
        .map_err(|e| format!("Failed to list drafts: {e}"))
}

#[tauri::command]
pub async fn get_draft(
    draft_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<Option<pod2_db::store::DraftInfo>, String> {
    let app_state = state.lock().await;

    pod2_db::store::get_draft(&app_state.db, &draft_id)
        .await
        .map_err(|e| format!("Failed to get draft: {e}"))
}

#[tauri::command]
pub async fn delete_draft(
    draft_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<bool, String> {
    let app_state = state.lock().await;

    pod2_db::store::delete_draft(&app_state.db, &draft_id)
        .await
        .map_err(|e| format!("Failed to delete draft: {e}"))
}

#[tauri::command]
pub async fn publish_draft(
    draft_id: String,
    server_url: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<PublishResult, String> {
    // First get the draft
    let draft = {
        let app_state = state.lock().await;
        pod2_db::store::get_draft(&app_state.db, &draft_id)
            .await
            .map_err(|e| format!("Failed to get draft: {e}"))?
            .ok_or("Draft not found")?
    };

    // Convert draft to publish parameters
    let file = if draft.content_type == "file" {
        draft
            .file_content
            .zip(draft.file_name)
            .zip(draft.file_mime_type)
            .map(|((content, name), mime_type)| DocumentFile {
                name,
                content,
                mime_type,
            })
    } else {
        None
    };

    let reply_to = draft.reply_to.and_then(|reply_str| {
        let parts: Vec<&str> = reply_str.split(':').collect();
        if parts.len() == 2 {
            if let (Ok(post_id), Ok(document_id)) =
                (parts[0].parse::<i64>(), parts[1].parse::<i64>())
            {
                return Some(ReplyReference {
                    post_id,
                    document_id,
                });
            }
        }
        None
    });

    // Call the existing publish_document function with draft_id for automatic deletion
    publish_document(
        draft.title,
        draft.message,
        file,
        draft.url,
        draft.tags,
        draft.authors,
        reply_to,
        server_url,
        Some(draft_id), // Pass draft_id for automatic deletion
        None,           // No post_id for draft publishing (creates new document)
        state,
    )
    .await
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteResult {
    pub success: bool,
    pub document_id: Option<i64>,
    pub error_message: Option<String>,
}

#[tauri::command]
pub async fn delete_document(
    document_id: i64,
    server_url: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<DeleteResult, String> {
    log::info!("Deleting document {document_id} from server {server_url}");

    // Get user's identity pod and private key from app state
    let app_state = state.lock().await;

    // Get the app setup state to get the username and identity pod ID
    let setup_state = pod2_db::store::get_app_setup_state(&app_state.db)
        .await
        .map_err(|e| format!("Failed to get app setup state: {e}"))?;

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

    log::info!("Looking for identity pod with ID: {identity_pod_id}");

    // Get the identity pod from the database
    let identity_pod_info = pod2_db::store::get_pod(&app_state.db, "identity", &identity_pod_id)
        .await
        .map_err(|e| format!("Failed to get identity pod: {e}"))?
        .ok_or(format!(
            "Identity pod not found in database with ID: {identity_pod_id}"
        ))?;

    let identity_pod: pod2::frontend::SignedPod = match identity_pod_info.data {
        PodData::Signed(pod) => (*pod)
            .try_into()
            .map_err(|e| format!("Failed to convert signed pod: {e}"))?,
        PodData::Main(_) => {
            return Err("Expected signed pod for identity, got main pod".to_string())
        }
    };

    // Verify the identity pod
    identity_pod
        .verify()
        .map_err(|e| format!("Identity pod verification failed: {e}"))?;
    log::info!("✓ Identity pod verification successful");

    // Get the user's private key
    let private_key = pod2_db::store::get_default_private_key_raw(&app_state.db)
        .await
        .map_err(|e| format!("Failed to get private key: {e}"))?;

    // First, fetch the document from server to get the actual document pod and timestamp pod
    log::info!("Fetching document {document_id} from server...");
    let client = reqwest::Client::new();
    let document_response = client
        .get(format!("{server_url}/documents/{document_id}"))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch document: {e}"))?;

    if !document_response.status().is_success() {
        return Ok(DeleteResult {
            success: false,
            document_id: Some(document_id),
            error_message: Some(format!(
                "Failed to fetch document: {}",
                document_response.status()
            )),
        });
    }

    let document: Document = document_response
        .json()
        .await
        .map_err(|e| format!("Failed to parse document response: {e}"))?;

    log::info!("✓ Document fetched successfully");

    // Extract only the timestamp pod from the server response
    // We'll create our own document pod for the delete request
    let timestamp_pod = document
        .pods
        .timestamp_pod
        .get()
        .map_err(|e| format!("Failed to get timestamp pod: {e}"))?;

    log::info!("✓ Timestamp pod extracted from server");

    // Verify the timestamp pod
    timestamp_pod
        .verify()
        .map_err(|e| format!("Timestamp pod verification failed: {e}"))?;
    log::info!("✓ Timestamp pod verification successful");

    // Extract the original data from the publish MainPod to use in delete pod
    let publish_main_pod = document
        .pods
        .pod
        .get()
        .map_err(|e| format!("Failed to get publish MainPod: {e}"))?;

    // The publish MainPod contains the verified data structure - we need to extract it
    // The data is in the public statements of the MainPod
    let publish_verified_statement = &publish_main_pod.public_statements[1]; // publish_verified statement
    let original_data = match publish_verified_statement {
        pod2::middleware::Statement::Custom(_, args) => &args[1], // Second argument is the data
        _ => return Err("Invalid MainPod structure - expected publish_verified statement".into()),
    };

    log::info!("✓ Original document data extracted from publish MainPod");

    // Create document pod for deletion request (signed by user) using the same data
    let params = Params::default();
    let mut delete_document_builder = SignedPodBuilder::new(&params);
    delete_document_builder.insert("request_type", "delete");
    delete_document_builder.insert("data", original_data.clone());
    delete_document_builder.insert("timestamp_pod", timestamp_pod.id());

    let delete_document_pod = delete_document_builder
        .sign(&Signer(private_key))
        .map_err(|e| format!("Failed to sign delete document pod: {e}"))?;

    // Verify the delete document pod
    delete_document_pod
        .verify()
        .map_err(|e| format!("Delete document pod verification failed: {e}"))?;
    log::info!("✓ Delete document pod created and verified");

    // Create main pod that proves both identity and document verification
    let delete_params = podnet_models::mainpod::delete::DeleteProofParams {
        identity_pod: &identity_pod,
        document_pod: &delete_document_pod,
        timestamp_pod,
        use_mock_proofs: false, // Use real proofs for production
    };
    let main_pod = podnet_models::mainpod::delete::prove_delete(delete_params)
        .map_err(|e| format!("Failed to generate delete verification MainPod: {e}"))?;

    log::info!("✓ Main pod created and verified");

    // Create the delete request
    let delete_request = DeleteRequest {
        document_id,
        username: username.clone(),
        main_pod,
    };

    log::info!("Sending delete request");
    let response = client
        .delete(format!("{server_url}/documents/{document_id}"))
        .header("Content-Type", "application/json")
        .json(&delete_request)
        .send()
        .await
        .map_err(|e| format!("Failed to send delete request: {e}"))?;

    if response.status().is_success() {
        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse delete response: {e}"))?;

        log::info!("✓ Successfully deleted document from server using main pod verification!");
        log::info!(
            "Server response: {}",
            serde_json::to_string_pretty(&result).unwrap_or_default()
        );

        // Trigger state sync to update the UI
        drop(app_state);
        let mut app_state = state.lock().await;
        if let Err(e) = app_state.trigger_state_sync().await {
            log::warn!("Failed to trigger state sync after delete: {e}");
        }

        Ok(DeleteResult {
            success: true,
            document_id: Some(document_id),
            error_message: None,
        })
    } else {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());

        log::error!("Delete request failed with status {status}: {error_text}");

        Ok(DeleteResult {
            success: false,
            document_id: Some(document_id),
            error_message: Some(format!("Server error: {status} - {error_text}")),
        })
    }
}

#[tauri::command]
pub async fn get_current_username(
    state: State<'_, Mutex<AppState>>,
) -> Result<Option<String>, String> {
    let app_state = state.lock().await;

    let setup_state = pod2_db::store::get_app_setup_state(&app_state.db)
        .await
        .map_err(|e| format!("Failed to get app setup state: {e}"))?;

    if !setup_state.setup_completed {
        return Ok(None);
    }

    Ok(setup_state.username)
}
