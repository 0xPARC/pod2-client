use anyhow::Result;
use pod2::middleware::TypedValue;
use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::Mutex;

use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubAuthUrlRequest {
    pub public_key: serde_json::Value,
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubAuthUrlResponse {
    pub auth_url: String,
    pub state: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubIdentityRequest {
    pub code: String,
    pub state: String,
    pub username: String,
    pub challenge_signature: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubIdentityResponse {
    pub identity_pod: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubIdentityPodResult {
    pub identity_pod: serde_json::Value,
    pub username: String,
    pub github_username: Option<String>,
    pub server_id: String,
}

/// Get GitHub OAuth authorization URL
#[tauri::command]
pub async fn get_github_auth_url(
    server_url: String,
    username: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<GitHubAuthUrlResponse, String> {
    log::info!("Getting GitHub OAuth authorization URL for user: {username}");

    // Get or create the user's private key during setup (same as regular identity setup)
    let app_state = state.lock().await;
    let private_key = match pod2_db::store::get_default_private_key_raw(&app_state.db).await {
        Ok(key) => {
            log::info!("Using existing default private key for GitHub OAuth");
            key
        }
        Err(_) => {
            log::info!("Creating new default private key for GitHub OAuth");
            pod2_db::store::create_default_private_key(&app_state.db)
                .await
                .map_err(|e| format!("Failed to create private key: {e}"))?
        }
    };

    let public_key = private_key.public_key();
    drop(app_state); // Release the lock before making HTTP requests

    let client = reqwest::Client::new();
    let request = GitHubAuthUrlRequest {
        public_key: serde_json::to_value(public_key)
            .map_err(|e| format!("Failed to serialize public key: {e}"))?,
        username: username.clone(),
    };

    let response = client
        .post(format!("{server_url}/auth/github"))
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Failed to request GitHub auth URL: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!(
            "GitHub auth URL request failed: {status} - {error_text}"
        ));
    }

    let auth_response: GitHubAuthUrlResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse GitHub auth URL response: {e}"))?;

    log::info!("Successfully obtained GitHub auth URL for user: {username}");
    Ok(auth_response)
}

/// Complete GitHub OAuth identity verification
#[tauri::command]
pub async fn complete_github_identity_verification(
    server_url: String,
    code: String,
    state: String,
    username: String,
    app_state: State<'_, Mutex<AppState>>,
) -> Result<GitHubIdentityPodResult, String> {
    log::info!("Completing GitHub OAuth identity verification for user: {username}");

    // Get or create the user's private key during setup
    let mut state_lock = app_state.lock().await;
    let _private_key = match pod2_db::store::get_default_private_key_raw(&state_lock.db).await {
        Ok(key) => {
            log::info!("Using existing default private key");
            key
        }
        Err(_) => {
            log::info!("Creating new default private key");
            pod2_db::store::create_default_private_key(&state_lock.db)
                .await
                .map_err(|e| format!("Failed to create private key: {e}"))?
        }
    };

    // For now, we'll use a placeholder challenge signature
    // In a full implementation, this would involve proper challenge signing
    let challenge_signature = "placeholder_signature".to_string();

    let client = reqwest::Client::new();
    let identity_request = GitHubIdentityRequest {
        code,
        state,
        username: username.clone(),
        challenge_signature,
    };

    let response = client
        .post(format!("{server_url}/identity"))
        .json(&identity_request)
        .send()
        .await
        .map_err(|e| format!("Failed to complete GitHub identity verification: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!(
            "GitHub identity verification failed: {status} - {error_text}"
        ));
    }

    let identity_response: GitHubIdentityResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse GitHub identity response: {e}"))?;

    // Convert the identity POD from JSON to the actual SignedPod type
    let identity_pod: pod2::frontend::SignedPod =
        serde_json::from_value(identity_response.identity_pod.clone())
            .map_err(|e| format!("Failed to deserialize identity POD: {e}"))?;

    // Extract GitHub username from the identity POD's github_data dictionary
    let github_username = identity_pod
        .get("github_data")
        .and_then(|v| match v.typed() {
            TypedValue::String(s) => {
                // Parse the GitHub data JSON
                if let Ok(github_data) = serde_json::from_str::<serde_json::Value>(s.as_str()) {
                    github_data
                        .get("github_username")
                        .and_then(|u| u.as_str())
                        .map(|u| u.to_string())
                } else {
                    None
                }
            }
            _ => None,
        });

    // Store identity POD in database as mandatory
    let pod_data = pod2_db::store::PodData::Signed(Box::new(identity_pod.clone().into()));
    let identity_pod_id = pod_data.id();

    // Ensure "identity" folder exists
    const IDENTITY_FOLDER: &str = "identity";
    if !pod2_db::store::space_exists(&state_lock.db, IDENTITY_FOLDER)
        .await
        .unwrap_or(false)
    {
        pod2_db::store::create_space(&state_lock.db, IDENTITY_FOLDER)
            .await
            .map_err(|e| format!("Failed to create identity folder: {e}"))?;
        log::info!("✓ Created identity folder");
    }

    pod2_db::store::store_identity_pod(
        &state_lock.db,
        &pod_data,
        IDENTITY_FOLDER,
        Some("GitHub Identity POD"),
    )
    .await
    .map_err(|e| format!("Failed to store identity POD: {e}"))?;

    // Update setup state with username and identity POD ID
    pod2_db::store::update_identity_info(&state_lock.db, &username, &identity_pod_id)
        .await
        .map_err(|e| format!("Failed to update identity info: {e}"))?;

    // Trigger state sync to refresh UI with new identity POD
    state_lock
        .trigger_state_sync()
        .await
        .map_err(|e| format!("Failed to trigger state sync: {e}"))?;

    log::info!("Successfully completed GitHub OAuth identity verification for user: {username}");

    Ok(GitHubIdentityPodResult {
        identity_pod: identity_response.identity_pod,
        username,
        github_username,
        server_id: "github-identity-server".to_string(),
    })
}

/// Detect if a server is a GitHub OAuth identity server
#[tauri::command]
pub async fn detect_github_oauth_server(
    server_url: String,
    _state: State<'_, Mutex<AppState>>,
) -> Result<bool, String> {
    log::info!("Detecting if server is GitHub OAuth server: {server_url}");

    let client = reqwest::Client::new();
    let response = client
        .get(&server_url)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to server: {e}"))?;

    if !response.status().is_success() {
        return Ok(false);
    }

    let server_info: serde_json::Value = response
        .json()
        .await
        .map_err(|_| "Failed to parse server response".to_string())?;

    // Check if server_id indicates GitHub OAuth server
    let is_github_server = server_info
        .get("server_id")
        .and_then(|v| v.as_str())
        .map(|id| id == "github-identity-server")
        .unwrap_or(false);

    log::info!("Server GitHub OAuth detection result: {is_github_server}");
    Ok(is_github_server)
}
