use anyhow::Context;
use config::FeatureConfig;
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
use tauri::{App, AppHandle, Emitter, Manager};
use tauri_plugin_store::StoreExt;
use tokio::sync::Mutex;

pub(crate) mod frog;

mod cache;
mod config;
mod features;
mod p2p;

const DEFAULT_SPACE_ID: &str = "default";

/// Get the feature configuration from environment variables
pub fn get_feature_config() -> FeatureConfig {
    FeatureConfig::load()
}

/// Tauri command to get the current feature configuration
#[tauri::command]
async fn get_feature_config_command() -> Result<FeatureConfig, String> {
    Ok(get_feature_config())
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
    p2p_node: Option<p2p::P2PNode>,
}

impl AppState {
    async fn refresh_pod_stats(&mut self) -> Result<(), String> {
        let total_pods = store::count_all_pods(&self.db)
            .await
            .map_err(|e| format!("Failed to count pods: {}", e))?;

        let (signed_pods, main_pods) = store::count_pods_by_type(&self.db)
            .await
            .map_err(|e| format!("Failed to count pods by type: {}", e))?;

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
            .map_err(|e| format!("Failed to emit state change: {}", e))?;
        Ok(())
    }

    async fn refresh_pod_lists(&mut self) -> Result<(), String> {
        // Load all PODs from all spaces for proper folder filtering
        let all_pods = store::list_all_pods(&self.db)
            .await
            .map_err(|e| format!("Failed to list all pods: {}", e))?;

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
            .map_err(|e| format!("Failed to list spaces: {}", e))?;

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
    let mut gov_signer = Signer(SecretKey(BigUint::from(1u32)));
    let mut pay_signer = Signer(SecretKey(BigUint::from(2u32)));
    let mut sanction_signer = Signer(SecretKey(BigUint::from(3u32)));

    let (gov_id_builder, pay_stub_builder, sanction_list_builder) =
        zu_kyc_sign_pod_builders(&params_for_test);

    let sign_results = [
        gov_id_builder.sign(&mut gov_signer),
        pay_stub_builder.sign(&mut pay_signer),
        sanction_list_builder.sign(&mut sanction_signer),
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
            log::error!("Failed to sign one or more pods for ZuKYC insertion: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

async fn init_db(path: &str) -> Result<Db, anyhow::Error> {
    log::info!("Initializing database at: {}", path);

    // Ensure the parent directory exists
    let path_buf = std::path::Path::new(path);
    if let Some(parent) = path_buf.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create parent directory for database: {:?}",
                parent
            )
        })?;
    }

    let db = Db::new(Some(path), &pod2_db::MIGRATIONS)
        .await
        .context("Failed to initialize database")?;

    setup_default_space(&db).await?;

    Ok(db)
}

fn set_default_config(app: &mut App, store_name: &str) {
    let store = app.store(store_name).unwrap();

    if store.get("instance_id").is_none() {
        store.set("instance_id", "default");
    }
}

async fn get_private_key(db: &Db) -> Result<SecretKey, String> {
    store::get_default_private_key(db)
        .await
        .map_err(|e| format!("Failed to get private key: {}", e))
}

#[tauri::command]
fn get_build_info() -> String {
    env!("GIT_COMMIT_HASH").to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_store::Builder::new().build());

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
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            tauri::async_runtime::block_on(async {
                let db_name = if let Ok(instance_id) = std::env::var("INSTANCE_ID") {
                    format!("app-data-{}.db", instance_id)
                } else {
                    "app-data.db".to_string()
                };
                let db_path = app.path().app_data_dir().unwrap().join(db_name);
                let db = init_db(db_path.to_str().unwrap())
                    .await
                    .expect("failed to initialize database");

                // Regenerate public keys if needed (fixes old hex format)
                store::regenerate_public_keys_if_needed(&db)
                    .await
                    .expect("failed to regenerate public keys");

                let store_name = if let Ok(instance_id) = std::env::var("INSTANCE_ID") {
                    format!("app-store-{}.json", instance_id)
                } else {
                    "app-store.json".to_string()
                };

                set_default_config(app, store_name.as_str());

                let app_handle = app.handle().clone();
                let mut app_state = AppState {
                    db,
                    state_data: AppStateData::default(),
                    app_handle,
                    p2p_node: None,
                };
                // Initialize state
                app_state
                    .trigger_state_sync()
                    .await
                    .expect("failed to initialize state");
                app.manage(Mutex::new(app_state));

                // Spawn cache warming task in background to avoid blocking startup
                tokio::task::spawn_blocking(|| {
                    cache::warm_mainpod_cache();
                });
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Build info commands
            get_build_info,
            // Frog commands
            frog::request_frog,
            // Configuration commands
            get_feature_config_command,
            // POD management commands
            pod_management::get_app_state,
            pod_management::trigger_sync,
            pod_management::delete_pod,
            pod_management::list_spaces,
            pod_management::import_pod,
            pod_management::insert_zukyc_pods,
            // Blockies commands
            blockies::commands::generate_blockies,
            blockies::commands::get_blockies_data,
            // Networking commands
            networking::start_p2p_node,
            networking::send_pod_to_peer,
            networking::send_message_as_pod,
            networking::get_inbox_messages,
            networking::accept_inbox_message,
            networking::get_chats,
            networking::get_chat_messages,
            // Authoring commands
            authoring::get_private_key_info,
            authoring::sign_pod,
            authoring::validate_code_command,
            authoring::execute_code_command,
            // Document commands
            documents::verify_document_pod,
            documents::upvote_document,
            documents::publish_document,
            // Identity setup commands
            identity_setup::setup_identity_server,
            identity_setup::register_username,
            identity_setup::complete_identity_setup,
            identity_setup::is_setup_completed,
            identity_setup::get_app_setup_state,
            // Integration commands
            integration::submit_pod_request
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
