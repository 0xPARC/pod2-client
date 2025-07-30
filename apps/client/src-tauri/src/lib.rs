use std::{path::PathBuf, str::FromStr};

use anyhow::Context;
use config::{AppConfig, FeatureConfig};
use features::{blockies, *};
use num::BigUint;
use pod2::{
    backends::plonky2::{primitives::ec::schnorr::SecretKey, signedpod::Signer},
    examples::zu_kyc_sign_pod_builders,
    frontend::SignedPod,
    middleware::Params,
};
use pod2_db::{
    store::{self, PodData, PodInfo, SpaceInfo},
    Db,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_log::{Target, TargetKind, TimezoneStrategy};
use tokio::sync::Mutex;

mod config;
mod features;
pub(crate) mod frog;

const DEFAULT_SPACE_ID: &str = "default";

/// Resolve database path with proper handling of absolute vs relative paths and name override
///
/// - If path is absolute: use as-is (ignore name override for backward compatibility)
/// - If path is the default "pod2.db": resolve against app data directory + use name override
/// - If path is a relative directory: resolve against current working directory + use name override
fn resolve_database_path(
    app_handle: &AppHandle,
    db_config: &config::DatabaseConfig,
) -> Result<PathBuf, String> {
    let path = std::path::Path::new(&db_config.path);

    // Handle absolute paths - use as-is (ignore name override for backward compatibility)
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }

    // Determine the base directory and apply name override
    let base_dir = if db_config.path == "pod2.db" {
        // Default case - use app data directory
        app_handle
            .path()
            .app_data_dir()
            .map_err(|e| format!("Failed to get app data directory: {e}"))?
    } else {
        // Custom relative path - treat as directory, resolve against current working directory
        std::env::current_dir()
            .map_err(|e| format!("Failed to get current working directory: {e}"))?
            .join(&db_config.path)
    };

    // Apply name override
    Ok(base_dir.join(&db_config.name))
}

/// Calculate the total size of a directory recursively
fn calculate_directory_size(path: &std::path::Path) -> Result<u64, String> {
    if !path.exists() {
        return Ok(0);
    }

    let mut total_size = 0u64;

    let entries = std::fs::read_dir(path)
        .map_err(|e| format!("Failed to read directory {}: {}", path.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {e}"))?;
        let path = entry.path();

        if path.is_dir() {
            total_size += calculate_directory_size(&path)?;
        } else {
            let metadata = std::fs::metadata(&path)
                .map_err(|e| format!("Failed to get metadata for {}: {}", path.display(), e))?;
            total_size += metadata.len();
        }
    }

    Ok(total_size)
}

/// Tauri command to get the current feature configuration
#[tauri::command]
async fn get_feature_config_command() -> Result<FeatureConfig, String> {
    Ok(config::config().features.clone())
}

/// Tauri command to get the full application configuration
#[tauri::command]
async fn get_app_config() -> Result<AppConfig, String> {
    Ok(config::config().clone())
}

/// Extended config info with full paths for debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedAppConfig {
    pub config: AppConfig,
    pub config_file_path: Option<String>,
    pub database_full_path: String,
}

/// Cache statistics for debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub cache_path: String,
    pub total_size_bytes: u64,
}

/// Tauri command to get extended app config with full paths
#[tauri::command]
async fn get_extended_app_config(app_handle: AppHandle) -> Result<ExtendedAppConfig, String> {
    let config = config::config().clone();

    // Get the full database path with proper resolution
    let database_full_path = resolve_database_path(&app_handle, &config.database)?
        .to_string_lossy()
        .to_string();

    // Try to determine config file path
    let config_file_path = std::env::var("POD2_CONFIG_FILE").ok().or_else(|| {
        // Try to find default config file location
        app_handle
            .path()
            .app_config_dir()
            .ok()
            .map(|dir| dir.join("config.toml").to_string_lossy().to_string())
    });

    Ok(ExtendedAppConfig {
        config,
        config_file_path,
        database_full_path,
    })
}

/// Tauri command to get cache statistics
#[tauri::command]
async fn get_cache_stats(app_handle: AppHandle) -> Result<CacheStats, String> {
    // Get the cache directory path using Tauri's path API
    let cache_base_dir = app_handle
        .path()
        .cache_dir()
        .map_err(|e| format!("Failed to get cache directory: {e}"))?;

    // POD2 cache is stored in a 'pod2' subdirectory
    let pod2_cache_dir = cache_base_dir.join("pod2");
    let cache_path = pod2_cache_dir.to_string_lossy().to_string();

    // Calculate total size of the cache directory
    let total_size_bytes = calculate_directory_size(&pod2_cache_dir).unwrap_or_else(|e| {
        log::warn!("Failed to calculate cache size: {e}");
        0
    });

    Ok(CacheStats {
        cache_path,
        total_size_bytes,
    })
}

/// Tauri command to clear the POD2 disk cache directory
#[tauri::command]
async fn clear_pod2_disk_cache(app_handle: AppHandle) -> Result<(), String> {
    // Get the cache directory path using Tauri's path API
    let cache_base_dir = app_handle
        .path()
        .cache_dir()
        .map_err(|e| format!("Failed to get cache directory: {e}"))?;

    // POD2 cache is stored in a 'pod2' subdirectory
    let pod2_cache_dir = cache_base_dir.join("pod2");

    if !pod2_cache_dir.exists() {
        log::info!("Cache directory does not exist, nothing to clear");
        return Ok(());
    }

    // Remove all contents of the cache directory
    let entries = std::fs::read_dir(&pod2_cache_dir)
        .map_err(|e| format!("Failed to read cache directory: {e}"))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read cache directory entry: {e}"))?;
        let path = entry.path();

        if path.is_dir() {
            std::fs::remove_dir_all(&path)
                .map_err(|e| format!("Failed to remove cache directory {}: {e}", path.display()))?;
        } else {
            std::fs::remove_file(&path)
                .map_err(|e| format!("Failed to remove cache file {}: {e}", path.display()))?;
        }
    }

    log::info!("POD2 disk cache directory cleared successfully");
    Ok(())
}

/// Tauri command to get a specific config section
#[tauri::command]
async fn get_config_section(section: String) -> Result<serde_json::Value, String> {
    let config = config::config();
    match section.as_str() {
        "features" => serde_json::to_value(&config.features)
            .map_err(|e| format!("Failed to serialize features: {e}")),
        "network" => serde_json::to_value(&config.network)
            .map_err(|e| format!("Failed to serialize network config: {e}")),
        "database" => serde_json::to_value(&config.database)
            .map_err(|e| format!("Failed to serialize database config: {e}")),
        "ui" => serde_json::to_value(&config.ui)
            .map_err(|e| format!("Failed to serialize UI config: {e}")),
        "logging" => serde_json::to_value(&config.logging)
            .map_err(|e| format!("Failed to serialize logging config: {e}")),
        _ => Err(format!("Unknown config section: {section}")),
    }
}

/// Tauri command to reload configuration from file (for hot reloading)
#[tauri::command]
async fn reload_config(
    app_handle: AppHandle,
    config_path: Option<String>,
) -> Result<AppConfig, String> {
    let path = config_path.map(PathBuf::from);
    let new_config = AppConfig::load_from_file(path)?;
    AppConfig::update(new_config.clone(), &app_handle)?;
    log::info!("Configuration reloaded successfully");
    Ok(new_config)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateData {
    pub pod_stats: PodStats,
    pub pod_lists: PodLists,
    pub spaces: Vec<SpaceInfo>,
    // Future state can be added here easily
    // pub user_preferences: UserPreferences,
    // pub recent_operations: Vec<Operation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodLists {
    pub signed_pods: Vec<PodInfo>,
    pub main_pods: Vec<PodInfo>,
}

impl PodLists {
    pub fn all_pods(&self) -> impl Iterator<Item = &PodInfo> {
        self.signed_pods.iter().chain(self.main_pods.iter())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodStats {
    pub total_pods: u32,
    pub signed_pods: u32,
    pub main_pods: u32,
}

impl Default for AppStateData {
    fn default() -> Self {
        Self {
            pod_stats: PodStats {
                total_pods: 0,
                signed_pods: 0,
                main_pods: 0,
            },
            pod_lists: PodLists {
                signed_pods: Vec::new(),
                main_pods: Vec::new(),
            },
            spaces: Vec::new(),
        }
    }
}

pub struct AppState {
    db: Db,
    state_data: AppStateData,
    app_handle: AppHandle,
}

impl AppState {
    async fn refresh_pod_stats(&mut self) -> Result<(), String> {
        let total_pods = store::count_all_pods(&self.db)
            .await
            .map_err(|e| format!("Failed to count pods: {e}"))?;

        let (signed_pods, main_pods) = store::count_pods_by_type(&self.db)
            .await
            .map_err(|e| format!("Failed to count pods by type: {e}"))?;

        self.state_data.pod_stats = PodStats {
            total_pods,
            signed_pods,
            main_pods,
        };

        Ok(())
    }

    async fn emit_state_change(&self) -> Result<(), String> {
        self.app_handle
            .emit("state-changed", &self.state_data)
            .map_err(|e| format!("Failed to emit state change: {e}"))?;
        Ok(())
    }

    async fn refresh_pod_lists(&mut self) -> Result<(), String> {
        // Load all PODs from all spaces for proper folder filtering
        let all_pods = store::list_all_pods(&self.db)
            .await
            .map_err(|e| format!("Failed to list all pods: {e}"))?;

        // Separate PODs by type for the frontend structure
        let signed_pods = all_pods
            .iter()
            .filter(|pod| pod.pod_type == "signed")
            .cloned()
            .collect();

        let main_pods = all_pods
            .iter()
            .filter(|pod| pod.pod_type == "main")
            .cloned()
            .collect();

        self.state_data.pod_lists = PodLists {
            signed_pods,
            main_pods,
        };

        Ok(())
    }

    async fn refresh_spaces(&mut self) -> Result<(), String> {
        let spaces = store::list_spaces(&self.db)
            .await
            .map_err(|e| format!("Failed to list spaces: {e}"))?;

        self.state_data.spaces = spaces;
        Ok(())
    }

    pub async fn trigger_state_sync(&mut self) -> Result<(), String> {
        // This can be called from anywhere to refresh all state
        self.refresh_pod_stats().await?;
        self.refresh_pod_lists().await?;
        self.refresh_spaces().await?;
        // Future: refresh other state components here

        // Always emit state change after sync
        self.emit_state_change().await?;
        Ok(())
    }
}

pub fn sign_zukyc_pods() -> anyhow::Result<Vec<SignedPod>> {
    let params_for_test = Params::default();
    let gov_signer = Signer(SecretKey(BigUint::from(1u32)));
    let pay_signer = Signer(SecretKey(BigUint::from(2u32)));

    let (gov_id_builder, pay_stub_builder) = zu_kyc_sign_pod_builders(&params_for_test);

    let sign_results = [
        gov_id_builder.sign(&gov_signer),
        pay_stub_builder.sign(&pay_signer),
    ];

    let all_signed: Result<Vec<_>, _> = sign_results.into_iter().collect();
    all_signed.map_err(|e| anyhow::anyhow!("Failed to sign Zukyc pods: {}", e))
}

pub async fn setup_default_space(db: &Db) -> anyhow::Result<()> {
    if store::space_exists(db, DEFAULT_SPACE_ID).await? {
        log::info!("Default space already exists. Skipping setup.");
        return Ok(());
    }

    log::info!("Setting up default space...");
    store::create_space(db, DEFAULT_SPACE_ID).await?;
    log::info!("Successfully set up default space.");

    Ok(())
}

pub async fn insert_zukyc_pods(db: &Db) -> anyhow::Result<()> {
    // Ensure default space exists
    if !store::space_exists(db, "zukyc").await? {
        store::create_space(db, "zukyc").await?;
    }

    log::info!("Inserting ZuKYC sample pods to default space...");

    match sign_zukyc_pods() {
        Ok(pods) => {
            log::info!("All pods signed successfully, importing to DB...");
            let pod_names = ["Gov ID", "Pay Stub", "Sanctions List"];

            for (pod, name) in pods.into_iter().zip(pod_names) {
                let pod_data = PodData::from(pod);
                store::import_pod(db, &pod_data, Some(name), "zukyc").await?;
            }
            log::info!("Successfully inserted ZuKYC pods to default space.");
        }
        Err(e) => {
            log::error!("Failed to sign one or more pods for ZuKYC insertion: {e}");
            return Err(e);
        }
    }

    Ok(())
}

async fn init_db(path: &str) -> Result<Db, anyhow::Error> {
    log::info!("Initializing database at: {path}");

    // Ensure the parent directory exists
    let path_buf = std::path::Path::new(path);
    if let Some(parent) = path_buf.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!("Failed to create parent directory for database: {parent:?}")
        })?;
    }

    let db = Db::new(Some(path), &pod2_db::MIGRATIONS)
        .await
        .context("Failed to initialize database")?;

    setup_default_space(&db).await?;

    Ok(db)
}

async fn get_private_key(db: &Db) -> Result<SecretKey, String> {
    store::get_default_private_key(db)
        .await
        .map_err(|e| format!("Failed to get private key: {e}"))
}

#[tauri::command]
fn get_build_info() -> String {
    env!("GIT_COMMIT_HASH").to_string()
}

/// Tauri command to reset the database - deletes current database and recreates it
#[tauri::command]
async fn reset_database(app_state: tauri::State<'_, Mutex<AppState>>) -> Result<(), String> {
    // Get the database config (need to clone to avoid holding the guard across await)
    let db_config = {
        let config = config::config();
        config.database.clone()
    };

    // Use tauri app handle to get proper app data directory
    let state_guard = app_state.lock().await;
    let app_handle = state_guard.app_handle.clone();
    drop(state_guard); // Release the lock before async operations

    let db_path = resolve_database_path(&app_handle, &db_config)?;

    log::info!("Resetting database at: {}", db_path.display());

    // Delete the existing database file if it exists
    if db_path.exists() {
        std::fs::remove_file(&db_path)
            .map_err(|e| format!("Failed to delete existing database: {e}"))?;
        log::info!("Deleted existing database file");
    }

    // Initialize a new database
    let new_db = init_db(db_path.to_str().unwrap())
        .await
        .map_err(|e| format!("Failed to recreate database: {e}"))?;

    // Update the app state with the new database
    let mut state_guard = app_state.lock().await;
    state_guard.db = new_db;

    // Reset the state data to default
    state_guard.state_data = AppStateData::default();

    // Trigger a full state sync to update the frontend
    state_guard
        .trigger_state_sync()
        .await
        .map_err(|e| format!("Failed to sync state after reset: {e}"))?;

    log::info!("Database reset completed successfully");
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_http::init());

    let debug = cfg!(dev);

    if !debug {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = app
                .get_webview_window("main")
                .expect("no main window")
                .set_focus();
        }));
    }

    builder
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_cli::init())
        .setup(|app| {
            tauri::async_runtime::block_on(async {
                // Initialize configuration system
                let config = {
                    use tauri_plugin_cli::CliExt;

                    let (config_path, cli_overrides) = match app.cli().matches() {
                        Ok(matches) => {
                            // Check for --config argument
                            let config_path = matches
                                .args
                                .get("config")
                                .and_then(|arg| arg.value.as_str())
                                .map(PathBuf::from)
                                .or_else(|| {
                                    std::env::var("POD2_CONFIG_FILE").ok().map(PathBuf::from)
                                });

                            // Extract --set arguments
                            let cli_overrides = matches
                                .args
                                .get("set")
                                .map(|arg| {
                                    match &arg.value {
                                        Value::Array(values) => {
                                            values.iter().filter_map(|v| v.as_str()).map(|s| s.to_string()).collect()
                                        },
                                        Value::String(value) => {
                                            vec![value.clone()]
                                        },
                                        _ => Vec::new()
                                    }
                                })
                                .unwrap_or_default();

                            (config_path, cli_overrides)
                        }
                        Err(e) => {
                            // The logger is not yet initialized, so we use eprintln.
                            eprintln!("Failed to parse CLI arguments: {e}");
                            // Fallback to environment variable
                            let config_path = std::env::var("POD2_CONFIG_FILE").ok().map(PathBuf::from);
                            (config_path, Vec::new())
                        }
                    };

                    match AppConfig::load_from_file(config_path) {
                        Ok(mut config) => {
                            // Apply CLI overrides
                            if !cli_overrides.is_empty() {
                                match config.apply_overrides(&cli_overrides) {
                                    Ok(()) => {
                                        eprintln!("Configuration loaded successfully with {} override(s).", cli_overrides.len());
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to apply config overrides: {e}");
                                        // Continue with config before overrides were applied
                                    }
                                }
                            } else {
                                eprintln!("Configuration loaded successfully.");
                            }
                            config
                        }

                        Err(e) => {
                            // The logger is not yet initialized, so we use eprintln.
                            eprintln!("Failed to load config file, using defaults: {e}");
                            let mut config = AppConfig::default();

                            // Still apply CLI overrides to default config
                            if !cli_overrides.is_empty() {
                                match config.apply_overrides(&cli_overrides) {
                                    Ok(()) => {
                                        eprintln!("Applied {} override(s) to default configuration.", cli_overrides.len());
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to apply config overrides to defaults: {e}");
                                    }
                                }
                            }

                            config
                        }
                    }
                };

                let log_level = log::LevelFilter::from_str(&config.logging.level)
                    .unwrap_or(log::LevelFilter::Info);

                let mut log_builder = tauri_plugin_log::Builder::new()
                    .level(log_level)
                    .timezone_strategy(TimezoneStrategy::UseLocal)
                    .clear_targets();

                // Add a file target to the default log directory.
                log_builder =
                    log_builder.target(Target::new(TargetKind::LogDir { file_name: None }));

                // Add a console target if enabled in the config.
                if config.logging.console_output {
                    log_builder = log_builder.target(Target::new(TargetKind::Stdout));
                }

                app.handle()
                    .plugin(log_builder.build())
                    .expect("failed to initialize logger");

                // Now that the logger is configured, we can use it.
                log::info!("Logger initialized. Configuration: {config:?}");

                // Initialize global configuration
                AppConfig::initialize(config.clone());

                // Use config for database path with proper resolution
                let db_path = resolve_database_path(app.handle(), &config.database)
                    .expect("Failed to resolve database path");
                let db = init_db(db_path.to_str().unwrap())
                    .await
                    .expect("failed to initialize database");

                // Regenerate public keys if needed (fixes old hex format)
                store::regenerate_public_keys_if_needed(&db)
                    .await
                    .expect("failed to regenerate public keys");

                let app_handle = app.handle().clone();
                let mut app_state = AppState {
                    db,
                    state_data: AppStateData::default(),
                    app_handle,
                };
                // Initialize state
                app_state
                    .trigger_state_sync()
                    .await
                    .expect("failed to initialize state");
                app.manage(Mutex::new(app_state));
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Build info commands
            get_build_info,
            // Debug commands
            reset_database,
            // Frog commands
            frog::request_frog,
            frog::request_score,
            frog::request_leaderboard,
            // Configuration commands
            get_feature_config_command,
            get_app_config,
            get_extended_app_config,
            get_config_section,
            reload_config,
            get_cache_stats,
            clear_pod2_disk_cache,
            // POD management commands
            pod_management::get_app_state,
            pod_management::trigger_sync,
            pod_management::delete_pod,
            pod_management::list_spaces,
            pod_management::import_pod,
            pod_management::insert_zukyc_pods,
            pod_management::pretty_print_custom_predicates,
            // Blockies commands
            blockies::commands::generate_blockies,
            blockies::commands::get_blockies_data,
            // Authoring commands
            authoring::get_private_key_info,
            authoring::sign_pod,
            authoring::validate_code_command,
            authoring::execute_code_command,
            // Document commands
            documents::verify_document_pod,
            documents::upvote_document,
            documents::publish_document,
            // Draft management commands
            documents::create_draft,
            documents::update_draft,
            documents::list_drafts,
            documents::get_draft,
            documents::delete_draft,
            documents::publish_draft,
            // Identity setup commands
            identity_setup::setup_identity_server,
            identity_setup::register_username,
            identity_setup::complete_identity_setup,
            identity_setup::is_setup_completed,
            identity_setup::get_app_setup_state,
            // GitHub OAuth identity setup commands
            identity_setup::get_github_auth_url,
            identity_setup::complete_github_identity_verification,
            identity_setup::detect_github_oauth_server,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
