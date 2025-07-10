use crate::{AppState, AppStateData};
use pod2_db::store;
use tauri::{State, AppHandle};
use tokio::sync::Mutex;

/// Get the current application state
#[tauri::command]
pub async fn get_app_state(state: State<'_, Mutex<AppState>>) -> Result<AppStateData, String> {
    let app_state = state.lock().await;
    Ok(app_state.state_data.clone())
}

/// Trigger a state synchronization
#[tauri::command]
pub async fn trigger_sync(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let mut app_state = state.lock().await;
    app_state.trigger_state_sync().await?;
    Ok(())
}

/// Set the pinned status of a POD
#[tauri::command]
pub async fn set_pod_pinned(
    state: State<'_, Mutex<AppState>>,
    space_id: String,
    pod_id: String,
    pinned: bool,
) -> Result<(), String> {
    let mut app_state = state.lock().await;

    store::set_pod_pinned(&app_state.db, &space_id, &pod_id, pinned)
        .await
        .map_err(|e| format!("Failed to set pod pinned status: {}", e))?;

    // Trigger state sync to update frontend
    app_state.trigger_state_sync().await?;

    Ok(())
}

/// List all spaces/folders
#[tauri::command]
pub async fn list_spaces(state: State<'_, Mutex<AppState>>) -> Result<Vec<serde_json::Value>, String> {
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
    use crate::{DEFAULT_SPACE_ID};
    use pod2_db::store::PodData;

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

/// Debug command to insert ZuKYC sample pods
#[tauri::command]
pub async fn insert_zukyc_pods(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    use crate::insert_zukyc_pods_to_default;
    
    let mut app_state = state.lock().await;

    insert_zukyc_pods_to_default(&app_state.db)
        .await
        .map_err(|e| format!("Failed to insert ZuKYC pods: {}", e))?;

    // Trigger state sync to update frontend
    app_state.trigger_state_sync().await?;

    Ok(())
}

/// Generate handler for pod management commands
pub fn pod_management_commands() -> impl Fn(tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    |builder| {
        builder.invoke_handler(tauri::generate_handler![
            get_app_state,
            trigger_sync,
            set_pod_pinned,
            list_spaces,
            import_pod,
            insert_zukyc_pods
        ])
    }
}