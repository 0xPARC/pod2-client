use pod2_db::store;
use tauri::State;
use tokio::sync::Mutex;

use crate::{config::config, AppState, AppStateData};

/// Macro to check if pod management feature is enabled
macro_rules! check_feature_enabled {
    () => {
        if !config().features.pod_management {
            log::warn!("Pod management feature is disabled");
            return Err("Pod management feature is disabled".to_string());
        }
    };
}

/// Get the current application state
#[tauri::command]
pub async fn get_app_state(state: State<'_, Mutex<AppState>>) -> Result<AppStateData, String> {
    check_feature_enabled!();
    let app_state = state.lock().await;
    Ok(app_state.state_data.clone())
}

/// Trigger a state synchronization
#[tauri::command]
pub async fn trigger_sync(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    check_feature_enabled!();
    let mut app_state = state.lock().await;
    app_state.trigger_state_sync().await?;
    Ok(())
}

/// List all spaces/folders
#[tauri::command]
pub async fn list_spaces(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<serde_json::Value>, String> {
    check_feature_enabled!();
    let app_state = state.lock().await;

    let spaces = store::list_spaces(&app_state.db)
        .await
        .map_err(|e| format!("Failed to list spaces: {}", e))?;

    Ok(spaces
        .into_iter()
        .map(|s| serde_json::to_value(s).unwrap())
        .collect())
}

/// Import a POD into the application
#[tauri::command]
pub async fn import_pod(
    state: State<'_, Mutex<AppState>>,
    serialized_pod: String,
    pod_type: String,
    label: Option<String>,
) -> Result<(), String> {
    check_feature_enabled!();
    use pod2_db::store::PodData;

    use crate::DEFAULT_SPACE_ID;

    let mut app_state = state.lock().await;

    let pod_data = match pod_type.as_str() {
        "Signed" => PodData::Signed(serde_json::from_str(&serialized_pod).unwrap()),
        "MockSigned" => PodData::Signed(serde_json::from_str(&serialized_pod).unwrap()),
        "Main" => PodData::Main(serde_json::from_str(&serialized_pod).unwrap()),
        "MockMain" => PodData::Main(serde_json::from_str(&serialized_pod).unwrap()),
        _ => return Err(format!("Not a valid POD type: {}", pod_type)),
    };

    let _ = store::import_pod(&app_state.db, &pod_data, label.as_deref(), DEFAULT_SPACE_ID)
        .await
        .map_err(|e| format!("Failed to import POD: {}", e));

    app_state.trigger_state_sync().await?;
    Ok(())
}

/// Delete a POD from the database
#[tauri::command]
pub async fn delete_pod(
    state: State<'_, Mutex<AppState>>,
    space_id: String,
    pod_id: String,
) -> Result<(), String> {
    check_feature_enabled!();
    let mut app_state = state.lock().await;

    let rows_deleted = store::delete_pod(&app_state.db, &space_id, &pod_id)
        .await
        .map_err(|e| format!("Failed to delete POD: {}", e))?;

    if rows_deleted == 0 {
        return Err("POD not found or already deleted".to_string());
    }

    // Trigger state sync to update frontend
    app_state.trigger_state_sync().await?;

    Ok(())
}

/// Debug command to insert ZuKYC sample pods
#[tauri::command]
pub async fn insert_zukyc_pods(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    check_feature_enabled!();
    use crate::insert_zukyc_pods;

    let mut app_state = state.lock().await;

    insert_zukyc_pods(&app_state.db)
        .await
        .map_err(|e| format!("Failed to insert ZuKYC pods: {}", e))?;

    // Trigger state sync to update frontend
    app_state.trigger_state_sync().await?;

    Ok(())
}
