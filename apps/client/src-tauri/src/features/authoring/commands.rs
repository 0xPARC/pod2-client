use std::collections::HashMap;

use pod2::{
    backends::plonky2::signedpod::Signer,
    frontend::SignedPodBuilder,
    middleware::{Params, Value as PodValue},
};
use pod2_db::store;
use tauri::{AppHandle, State};
use tokio::sync::Mutex;

use crate::{get_feature_config, AppState};

/// Macro to check if authoring feature is enabled
macro_rules! check_feature_enabled {
    () => {
        if !get_feature_config().authoring {
            log::warn!("Authoring feature is disabled");
            return Err("Authoring feature is disabled".to_string());
        }
    };
}

/// Get information about the default private key
#[tauri::command]
pub async fn get_private_key_info(
    state: State<'_, Mutex<AppState>>,
) -> Result<serde_json::Value, String> {
    check_feature_enabled!();
    let app_state = state.lock().await;

    store::get_default_private_key_info(&app_state.db)
        .await
        .map_err(|e| format!("Failed to get private key info: {}", e))
}

/// Sign a POD with the given key-value pairs
#[tauri::command]
pub async fn sign_pod(
    state: State<'_, Mutex<AppState>>,
    serialized_pod_values: String,
) -> Result<String, String> {
    let app_state = state.lock().await;

    let kvs: HashMap<String, PodValue> = serde_json::from_str(&serialized_pod_values)
        .map_err(|e| format!("Failed to parse serialized pod values: {}", e))?;

    let params = Params::default();
    let mut builder = SignedPodBuilder::new(&params);
    for (key, value) in kvs {
        builder.insert(key, value);
    }

    // Get default private key (auto-created if needed)
    let private_key = store::get_default_private_key(&app_state.db)
        .await
        .map_err(|e| format!("Failed to get private key: {}", e))?;

    let mut signer = Signer(private_key);

    let signed_pod = builder
        .sign(&mut signer)
        .map_err(|e| format!("Failed to sign pod: {}", e))?;

    Ok(serde_json::to_string(&signed_pod).unwrap())
}

/// Generate handler for authoring commands
pub fn authoring_commands() -> impl Fn(tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    |builder| builder.invoke_handler(tauri::generate_handler![get_private_key_info, sign_pod])
}
