use std::sync::Arc;

use pod2::{
    frontend::MainPod,
    lang::pretty_print::PrettyPrint,
    middleware::{CustomPredicateBatch, Predicate},
};
use pod2_db::store;
use tauri::State;
use tokio::sync::Mutex;

use crate::{AppState, AppStateData};

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

/// List all spaces/folders
#[tauri::command]
pub async fn list_spaces(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<serde_json::Value>, String> {
    let app_state = state.lock().await;

    let spaces = store::list_spaces(&app_state.db)
        .await
        .map_err(|e| format!("Failed to list spaces: {e}"))?;

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
    use pod2_db::store::PodData;

    use crate::DEFAULT_SPACE_ID;

    let mut app_state = state.lock().await;

    let pod_data = match pod_type.as_str() {
        "Signed" => PodData::Signed(
            serde_json::from_str(&serialized_pod)
                .map_err(|e| format!("Failed to deserialize signed dict: {e}"))?,
        ),
        "MockSigned" => PodData::Signed(
            serde_json::from_str(&serialized_pod)
                .map_err(|e| format!("Failed to deserialize signed dict: {e}"))?,
        ),
        "Main" => PodData::Main(
            serde_json::from_str(&serialized_pod)
                .map_err(|e| format!("Failed to deserialize main pod: {e}"))?,
        ),
        "MockMain" => PodData::Main(
            serde_json::from_str(&serialized_pod)
                .map_err(|e| format!("Failed to deserialize main pod: {e}"))?,
        ),
        _ => return Err(format!("Not a valid POD type: {pod_type}")),
    };

    let _ = store::import_pod(&app_state.db, &pod_data, label.as_deref(), DEFAULT_SPACE_ID)
        .await
        .map_err(|e| format!("Failed to import POD: {e}"));

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
    let mut app_state = state.lock().await;

    let rows_deleted = store::delete_pod(&app_state.db, &space_id, &pod_id)
        .await
        .map_err(|e| format!("Failed to delete POD: {e}"))?;

    if rows_deleted == 0 {
        return Err("POD not found or already deleted".to_string());
    }

    // Trigger state sync to update frontend
    app_state.trigger_state_sync().await?;

    Ok(())
}

// /// Debug command to insert ZuKYC sample pods
// #[tauri::command]
// pub async fn insert_zukyc_pods(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
//     use crate::insert_zukyc_pods;

//     let mut app_state = state.lock().await;

//     insert_zukyc_pods(&app_state.db)
//         .await
//         .map_err(|e| format!("Failed to insert ZuKYC pods: {e}"))?;

//     // Trigger state sync to update frontend
//     app_state.trigger_state_sync().await?;

//     Ok(())
// }

/// Return pretty-printed Podlang for custom predicates
#[tauri::command]
pub async fn pretty_print_custom_predicates(serialized_main_pod: String) -> Result<String, String> {
    let main_pod: MainPod = serde_json::from_str(&serialized_main_pod).unwrap();

    let batches = main_pod
        .public_statements
        .iter()
        .filter_map(|statement| match statement.predicate() {
            Predicate::Custom(custom_predicate) => Some(custom_predicate.batch.clone()),
            _ => None,
        })
        .flat_map(|batch| {
            let mut collected_batches: Vec<_> = batch
                .predicates()
                .iter()
                .flat_map(|pred| pred.statements().iter())
                .filter_map(|stmt| {
                    if let Predicate::Custom(inner_custom_predicate) = &stmt.pred {
                        Some(inner_custom_predicate.batch.clone())
                    } else {
                        None
                    }
                })
                .collect();
            collected_batches.push(batch);
            collected_batches
        })
        .collect::<Vec<Arc<CustomPredicateBatch>>>();

    Ok(batches
        .iter()
        .map(|batch| batch.to_podlang_string())
        .collect::<Vec<String>>()
        .join("\n\n"))
}
