use pod2_db::store;
use tauri::State;
use tokio::sync::Mutex;

use crate::{
    features::console::commands::{
        log_error_event_from_app_handle, log_pod_operation_from_app_handle,
    },
    get_feature_config, AppState, AppStateData,
};

/// Format size in bytes to human-readable format
fn format_size_bytes(bytes: usize) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    const THRESHOLD: f64 = 1024.0;

    if bytes == 0 {
        return "0B".to_string();
    }

    let bytes_f = bytes as f64;
    let mut size = bytes_f;
    let mut unit_index = 0;

    while size >= THRESHOLD && unit_index < UNITS.len() - 1 {
        size /= THRESHOLD;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{}{}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1}{}", size, UNITS[unit_index])
    }
}

/// Macro to check if pod management feature is enabled
macro_rules! check_feature_enabled {
    () => {
        if !get_feature_config().pod_management {
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

/// List all folders/spaces
#[tauri::command]
pub async fn list_spaces(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<serde_json::Value>, String> {
    check_feature_enabled!();
    let app_state = state.lock().await;

    let spaces = store::list_spaces(&app_state.db)
        .await
        .map_err(|e| format!("Failed to list folders: {}", e))?;

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
    let app_handle = app_state.app_handle.clone();

    let pod_data = match pod_type.as_str() {
        "Signed" => PodData::Signed(serde_json::from_str(&serialized_pod).map_err(|e| {
            let error_msg = format!("Failed to parse signed POD: {}", e);
            let app_handle_clone = app_handle.clone();
            tokio::spawn(async move {
                log_error_event_from_app_handle(
                    &app_handle_clone,
                    format!("❌ Import failed: Invalid signed POD format"),
                )
                .await;
            });
            error_msg
        })?),
        "MockSigned" => PodData::Signed(serde_json::from_str(&serialized_pod).map_err(|e| {
            let error_msg = format!("Failed to parse mock signed POD: {}", e);
            let app_handle_clone = app_handle.clone();
            tokio::spawn(async move {
                log_error_event_from_app_handle(
                    &app_handle_clone,
                    format!("❌ Import failed: Invalid mock signed POD format"),
                )
                .await;
            });
            error_msg
        })?),
        "Main" => PodData::Main(serde_json::from_str(&serialized_pod).map_err(|e| {
            let error_msg = format!("Failed to parse main POD: {}", e);
            let app_handle_clone = app_handle.clone();
            tokio::spawn(async move {
                log_error_event_from_app_handle(
                    &app_handle_clone,
                    format!("❌ Import failed: Invalid main POD format"),
                )
                .await;
            });
            error_msg
        })?),
        "MockMain" => PodData::Main(serde_json::from_str(&serialized_pod).map_err(|e| {
            let error_msg = format!("Failed to parse mock main POD: {}", e);
            let app_handle_clone = app_handle.clone();
            tokio::spawn(async move {
                log_error_event_from_app_handle(
                    &app_handle_clone,
                    format!("❌ Import failed: Invalid mock main POD format"),
                )
                .await;
            });
            error_msg
        })?),
        _ => {
            let error_msg = format!("Not a valid POD type: {}", pod_type);
            let app_handle_clone = app_handle.clone();
            let pod_type_clone = pod_type.clone();
            tokio::spawn(async move {
                log_error_event_from_app_handle(
                    &app_handle_clone,
                    format!("❌ Import failed: Invalid POD type '{}'", pod_type_clone),
                )
                .await;
            });
            return Err(error_msg);
        }
    };

    // Get POD ID and size for logging
    let pod_id = pod_data.id();
    let pod_id_short = pod_id[..8.min(pod_id.len())].to_string();
    let size_bytes = pod_data.calculate_size_bytes();
    let size_str = format_size_bytes(size_bytes);

    match store::import_pod(&app_state.db, &pod_data, label.as_deref(), DEFAULT_SPACE_ID).await {
        Ok(()) => {
            // Log successful import
            let import_msg = if let Some(ref label_str) = label {
                format!(
                    "📥 POD imported via GUI: \"{}\" ({}, {}) → {}/",
                    label_str, pod_id_short, size_str, DEFAULT_SPACE_ID
                )
            } else {
                format!(
                    "📥 POD imported via GUI: {} POD ({}, {}) → {}/",
                    pod_type, pod_id_short, size_str, DEFAULT_SPACE_ID
                )
            };

            let app_handle_clone = app_handle.clone();
            tokio::spawn(async move {
                log_pod_operation_from_app_handle(&app_handle_clone, import_msg).await;
            });
        }
        Err(e) => {
            let error_msg = format!("Failed to import POD: {}", e);
            let app_handle_clone = app_handle.clone();
            let import_label = label.clone().unwrap_or_else(|| format!("{} POD", pod_type));
            tokio::spawn(async move {
                log_error_event_from_app_handle(
                    &app_handle_clone,
                    format!("❌ Import failed: {} ({})", import_label, e),
                )
                .await;
            });
            return Err(error_msg);
        }
    }

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
    let app_handle = app_state.app_handle.clone();

    // Try to get POD info before deletion for better logging
    let pod_info = store::get_pod(&app_state.db, &space_id, &pod_id)
        .await
        .ok()
        .flatten();
    let pod_id_short = pod_id[..8.min(pod_id.len())].to_string();

    match store::delete_pod(&app_state.db, &space_id, &pod_id).await {
        Ok(rows_deleted) => {
            if rows_deleted == 0 {
                let error_msg = "POD not found or already deleted".to_string();
                let app_handle_clone = app_handle.clone();
                let space_id_clone = space_id.clone();
                tokio::spawn(async move {
                    log_error_event_from_app_handle(
                        &app_handle_clone,
                        format!(
                            "❌ Delete failed: POD {} not found in folder {}",
                            pod_id_short, space_id_clone
                        ),
                    )
                    .await;
                });
                return Err(error_msg);
            }

            // Log successful deletion
            let delete_msg = if let Some(info) = pod_info {
                let size_str = format_size_bytes(info.size_bytes);
                if let Some(label) = info.label {
                    format!(
                        "🗑️ POD deleted via GUI: {} (\"{}\", {}) from {}/",
                        pod_id_short, label, size_str, space_id
                    )
                } else {
                    format!(
                        "🗑️ POD deleted via GUI: {} ({} POD, {}) from {}/",
                        pod_id_short, info.pod_type, size_str, space_id
                    )
                }
            } else {
                format!(
                    "🗑️ POD deleted via GUI: {} from {}/",
                    pod_id_short, space_id
                )
            };

            let app_handle_clone = app_handle.clone();
            tokio::spawn(async move {
                log_pod_operation_from_app_handle(&app_handle_clone, delete_msg).await;
            });
        }
        Err(e) => {
            let error_msg = format!("Failed to delete POD: {}", e);
            let app_handle_clone = app_handle.clone();
            let space_id_clone = space_id.clone();
            tokio::spawn(async move {
                log_error_event_from_app_handle(
                    &app_handle_clone,
                    format!(
                        "❌ Delete failed: POD {} from {} ({})",
                        pod_id_short, space_id_clone, e
                    ),
                )
                .await;
            });
            return Err(error_msg);
        }
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
