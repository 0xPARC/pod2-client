use std::sync::Arc;

use tauri::{AppHandle, Manager, State};

use super::{
    service::ConsoleService,
    types::{ConsoleEvent, ConsoleMessage, ConsoleState},
};

/// Console service state for Tauri commands
pub type ConsoleServiceState = Arc<ConsoleService>;

/// Macro to check if console feature is enabled (placeholder - console always enabled for now)
macro_rules! check_console_enabled {
    () => {
        // TODO: Add feature flag check when console feature flag is added
    };
}

/// Execute a console command
#[tauri::command]
pub async fn console_execute_command(
    console_service: State<'_, ConsoleServiceState>,
    input: String,
) -> Result<String, String> {
    check_console_enabled!();
    console_service.execute_command(input).await
}

/// Get recent console messages
#[tauri::command]
pub async fn console_get_messages(
    console_service: State<'_, ConsoleServiceState>,
    limit: Option<usize>,
) -> Result<Vec<ConsoleMessage>, String> {
    check_console_enabled!();
    Ok(console_service.get_messages(limit).await)
}

/// Get current console state
#[tauri::command]
pub async fn console_get_state(
    console_service: State<'_, ConsoleServiceState>,
) -> Result<ConsoleState, String> {
    check_console_enabled!();
    Ok(console_service.get_state().await)
}

/// Get command history for autocomplete/navigation
#[tauri::command]
pub async fn console_get_command_history(
    console_service: State<'_, ConsoleServiceState>,
) -> Result<Vec<String>, String> {
    check_console_enabled!();
    Ok(console_service.get_command_history().await)
}

/// Log GUI event to console
#[tauri::command]
pub async fn console_log_event(
    console_service: State<'_, ConsoleServiceState>,
    event: ConsoleEvent,
) -> Result<(), String> {
    check_console_enabled!();
    console_service.log_gui_event(event).await;
    Ok(())
}

/// Initialize console service
pub fn init_console_service(app_handle: AppHandle) -> ConsoleServiceState {
    Arc::new(ConsoleService::new(app_handle))
}

// =============================================================================
// Helper Functions for GUI Event Logging
// =============================================================================

/// Helper function for other modules to log POD operations
pub async fn log_pod_operation_from_app_handle(app_handle: &AppHandle, message: String) {
    if let Some(console_service) = app_handle.try_state::<ConsoleServiceState>() {
        console_service.log_pod_operation(message).await;
    }
}

/// Helper function for other modules to log system events
pub async fn log_system_event_from_app_handle(app_handle: &AppHandle, message: String) {
    if let Some(console_service) = app_handle.try_state::<ConsoleServiceState>() {
        console_service.log_system_event(message).await;
    }
}

/// Helper function for other modules to log error events
pub async fn log_error_event_from_app_handle(app_handle: &AppHandle, message: String) {
    if let Some(console_service) = app_handle.try_state::<ConsoleServiceState>() {
        console_service.log_error_event(message).await;
    }
}
