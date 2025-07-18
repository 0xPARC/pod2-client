use std::collections::HashMap;

use anyhow::Result;
use pod2::middleware::TypedValue;
use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::Mutex;

use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct IdentityServerInfo {
    pub server_id: String,
    pub public_key: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IdentityPodResult {
    pub identity_pod: serde_json::Value,
    pub username: String,
    pub server_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct IdentityRequest {
    server_challenge_pod: pod2::frontend::SignedPod,
    user_response_pod: pod2::frontend::SignedPod,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChallengeResponse {
    challenge_pod: pod2::frontend::SignedPod,
}

#[derive(Debug, Serialize, Deserialize)]
struct IdentityResponse {
    identity_pod: pod2::frontend::SignedPod,
}

/// Configure and validate connection to an identity server
#[tauri::command]
pub async fn setup_identity_server(
    server_url: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<IdentityServerInfo, String> {
    log::info!("Setting up identity server: {}", server_url);

    // Make HTTP GET request to identity server's root endpoint
    let client = reqwest::Client::new();
    let response = client
        .get(&server_url)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to identity server: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Identity server returned error: {}",
            response.status()
        ));
    }

    let server_info_raw = response
        .json::<HashMap<String, serde_json::Value>>()
        .await
        .map_err(|e| format!("Failed to parse identity server response: {}", e))?;

    // Extract server_id and public_key from response
    let server_id = server_info_raw
        .get("server_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing server_id in identity server response")?
        .to_string();

    let public_key = server_info_raw
        .get("public_key")
        .cloned()
        .ok_or("Missing public_key in identity server response")?;

    // Store server info in database
    let public_key_str = serde_json::to_string(&public_key)
        .map_err(|e| format!("Failed to serialize public key: {}", e))?;

    let app_state = state.lock().await;
    pod2_db::store::update_identity_server_info(
        &app_state.db,
        &server_url,
        &server_id,
        &public_key_str,
    )
    .await
    .map_err(|e| format!("Failed to store identity server info: {}", e))?;

    log::info!("Successfully configured identity server: {}", server_id);

    Ok(IdentityServerInfo {
        server_id,
        public_key,
    })
}

/// Register a username with the identity server and obtain an identity POD
#[tauri::command]
pub async fn register_username(
    username: String,
    server_url: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<IdentityPodResult, String> {
    log::info!("Registering username '{}' with identity server", username);

    // Get or create the user's private key during setup
    let mut app_state = state.lock().await;
    let private_key = match pod2_db::store::get_default_private_key_raw(&app_state.db).await {
        Ok(key) => {
            log::info!("Using existing default private key");
            key
        }
        Err(_) => {
            log::info!("Creating new default private key");
            pod2_db::store::create_default_private_key(&app_state.db)
                .await
                .map_err(|e| format!("Failed to create private key: {}", e))?
        }
    };

    let public_key = private_key.public_key();

    // Step 1: Request challenge from identity server
    let client = reqwest::Client::new();
    let challenge_response = client
        .post(format!("{}/user/challenge", server_url))
        .json(&serde_json::json!({
            "username": username,
            "user_public_key": public_key
        }))
        .send()
        .await
        .map_err(|e| format!("Failed to request challenge: {}", e))?;

    if !challenge_response.status().is_success() {
        return Err(format!(
            "Challenge request failed: {}",
            challenge_response.status()
        ));
    }

    let challenge_response_text = challenge_response
        .text()
        .await
        .map_err(|e| format!("Failed to read challenge response text: {}", e))?;

    let challenge_data: ChallengeResponse = serde_json::from_str(&challenge_response_text)
        .map_err(|e| {
            log::error!(
                "Failed to parse challenge response: {}, raw response: {}",
                e,
                challenge_response_text
            );
            format!(
                "Failed to parse challenge response: {}, raw response: {}",
                e, challenge_response_text
            )
        })?;

    let challenge_pod = challenge_data.challenge_pod;

    // Verify the challenge pod signature
    challenge_pod
        .verify()
        .map_err(|e| format!("Failed to verify challenge pod: {}", e))?;

    let challenge_string = challenge_pod
        .get("challenge")
        .and_then(|v| match v.typed() {
            TypedValue::String(s) => Some(s.as_str()),
            _ => None,
        })
        .ok_or("Missing challenge string")?;

    // Create and sign user response pod
    let params = pod2::middleware::Params::default();
    let mut challenge_builder = pod2::frontend::SignedPodBuilder::new(&params);
    challenge_builder.insert("challenge", challenge_string);
    challenge_builder.insert("username", username.as_str());

    let mut user_signer = pod2::backends::plonky2::signedpod::Signer(private_key);
    let user_response_pod = challenge_builder
        .sign(&mut user_signer)
        .map_err(|e| format!("Failed to sign challenge response: {}", e))?;

    // Step 3: Submit challenge response to get identity POD
    let identity_request = IdentityRequest {
        server_challenge_pod: challenge_pod,
        user_response_pod,
    };

    let identity_response = client
        .post(format!("{}/identity", server_url))
        .json(&identity_request)
        .send()
        .await
        .map_err(|e| format!("Failed to submit identity verification: {}", e))?;

    if !identity_response.status().is_success() {
        return Err(format!(
            "Identity verification failed: {}",
            identity_response.status()
        ));
    }

    let identity_data = identity_response
        .json::<IdentityResponse>()
        .await
        .map_err(|e| format!("Failed to parse identity response: {}", e))?;

    let identity_pod = identity_data.identity_pod;

    // Step 4: Store identity POD in database as mandatory
    let identity_pod_id = "identity_pod"; // TODO: Generate proper ID from POD
    let pod_data = pod2_db::store::PodData::Signed(identity_pod.clone().into());

    pod2_db::store::store_identity_pod(&app_state.db, &pod_data, "default", Some("Identity POD"))
        .await
        .map_err(|e| format!("Failed to store identity POD: {}", e))?;

    // Step 5: Update setup state with username and identity POD ID
    pod2_db::store::update_identity_info(&app_state.db, &username, identity_pod_id)
        .await
        .map_err(|e| format!("Failed to update identity info: {}", e))?;

    // Step 6: Trigger state sync to refresh UI with new identity POD
    app_state.trigger_state_sync().await?;

    log::info!(
        "Successfully registered username '{}' and received identity POD",
        username
    );

    Ok(IdentityPodResult {
        identity_pod: serde_json::to_value(identity_pod)
            .map_err(|e| format!("Failed to serialize identity POD: {}", e))?,
        username,
        server_id: "identity_server".to_string(), // TODO: Get from stored server info
    })
}

/// Complete the identity setup process and mark as finished
#[tauri::command]
pub async fn complete_identity_setup(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    log::info!("Completing identity setup");

    // Mark setup as completed in database
    let app_state = state.lock().await;
    pod2_db::store::complete_app_setup(&app_state.db)
        .await
        .map_err(|e| format!("Failed to complete setup: {}", e))?;

    log::info!("Identity setup completed successfully");

    Ok(())
}

/// Check if the app setup has been completed
#[tauri::command]
pub async fn is_setup_completed(state: State<'_, Mutex<AppState>>) -> Result<bool, String> {
    let app_state = state.lock().await;
    pod2_db::store::is_setup_completed(&app_state.db)
        .await
        .map_err(|e| format!("Failed to check setup status: {}", e))
}

/// Get the current app setup state
#[tauri::command]
pub async fn get_app_setup_state(
    state: State<'_, Mutex<AppState>>,
) -> Result<pod2_db::store::AppSetupState, String> {
    let app_state = state.lock().await;
    pod2_db::store::get_app_setup_state(&app_state.db)
        .await
        .map_err(|e| format!("Failed to get setup state: {}", e))
}
