use std::collections::HashMap;

use anyhow::Context;
use chrono::Utc;
use hex::ToHex;
use num::BigUint;
use pod2::{
    backends::plonky2::{
        mainpod::Prover, mock::mainpod::MockProver, primitives::ec::schnorr::SecretKey,
        signedpod::Signer,
    },
    examples::{zu_kyc_sign_pod_builders, MOCK_VD_SET},
    frontend::{
        MainPod, MainPodBuilder, SerializedMainPod, SerializedSignedPod, SignedPod,
        SignedPodBuilder,
    },
    lang,
    middleware::{Params, PodId, PodType, Value as PodValue, DEFAULT_VD_SET},
};
use pod2_db::{
    store::{self, PodData, PodInfo},
    Db,
};
use pod2_solver::{db::IndexablePod, metrics::MetricsLevel};
use serde::{Deserialize, Serialize};
use tauri::{App, AppHandle, Emitter, Manager, State};
use tauri_plugin_store::StoreExt;
use tokio::sync::Mutex;

mod p2p;

#[tauri::command]
async fn submit_pod_request(
    state: State<'_, Mutex<AppState>>,
    request: String,
) -> Result<SerializedMainPod, String> {
    log::info!("request: {}", request);
    let params = Params::default();
    let pod_request = lang::parse(request.as_str(), &params, &[]).unwrap();

    #[allow(unused_variables)]
    let mock = false;
    #[cfg(debug_assertions)]
    let mock = true;

    let mut app_state = state.lock().await;
    let fetched_pod_infos = store::list_pods(&app_state.db, "zukyc")
        .await
        .map_err(|e| format!("Failed to list pods: {}", e))?;

    let mut owned_signed_pods: Vec<SignedPod> = Vec::new();
    let mut owned_main_pods: Vec<MainPod> = Vec::new();

    for pod_info in fetched_pod_infos {
        // Sanity check: Ensure the pod_type string from DB matches the PodData enum variant type
        if pod_info.pod_type != pod_info.data.type_str() {
            log::warn!(
                "Data inconsistency for pod_id '{}' in space '{}' during execution: DB pod_type is '{}' but deserialized PodData is for '{}'. Trusting PodData enum.",
                pod_info.id, "zukyc", pod_info.pod_type, pod_info.data.type_str()
            );
            // If they mismatch, we should probably trust the actual data content (the enum variant)
            // but it indicates a potential issue elsewhere (e.g., during import or manual DB edit).
        }

        match pod_info.data {
            PodData::Signed(helper) => {
                owned_signed_pods.push(SignedPod::try_from(helper).unwrap());
            }
            PodData::Main(helper) => match MainPod::try_from(helper) {
                Ok(main_pod) => {
                    owned_main_pods.push(main_pod);
                }
                Err(e) => {
                    log::error!(
                        "Failed to convert MainPodHelper to MainPod (id: {}, space: {}): {:?}",
                        pod_info.id,
                        "zukyc",
                        e
                    );
                    return Err(format!(
                        "Failed to convert MainPodHelper to MainPod (id: {}, space: {}): {:?}",
                        pod_info.id, "zukyc", e
                    ));
                }
            },
        }
    }

    let mut all_pods_for_facts: Vec<IndexablePod> = Vec::new();
    let mut original_signed_pods: HashMap<PodId, &SignedPod> = HashMap::new();
    let mut original_main_pods: HashMap<PodId, &MainPod> = HashMap::new();

    for signed_pod_ref in &owned_signed_pods {
        // If not in mock mode, Signed PODs must be of type Signed.
        if !mock && signed_pod_ref.pod.pod_type().0 != PodType::Signed as usize {
            continue;
        }
        all_pods_for_facts.push(IndexablePod::signed_pod(signed_pod_ref));
        original_signed_pods.insert(signed_pod_ref.id(), signed_pod_ref);
    }

    for main_pod_ref in &owned_main_pods {
        // If not in mock mode, Main PODs must be of type Main.
        if !mock && main_pod_ref.pod.pod_type().0 != PodType::Main as usize {
            continue;
        }
        all_pods_for_facts.push(IndexablePod::main_pod(main_pod_ref));
        original_main_pods.insert(main_pod_ref.id(), main_pod_ref);
    }

    // let initial_facts = facts_from_pods(&all_pods_for_facts);
    // let custom_definitions =
    //     custom_definitions_from_batches(&[processed_output.custom_batch], &params);
    let request_templates = pod_request.request_templates;

    let (proof, _) =
        match pod2_solver::solve(&request_templates, &all_pods_for_facts, MetricsLevel::None) {
            Ok(solution) => solution,
            Err(e) => {
                log::error!("Solver error: {:?}", e);
                return Err(format!("Solver error: {:?}, request: {}", e, request));
            }
        };

    let (pod_ids, ops) = proof.to_inputs();

    let vd_set = if mock {
        #[allow(clippy::borrow_interior_mutable_const)]
        &*MOCK_VD_SET
    } else {
        &*DEFAULT_VD_SET
    };

    let mut builder = MainPodBuilder::new(&params, vd_set);
    for (operation, public) in ops {
        if public {
            builder.pub_op(operation).unwrap();
        } else {
            builder.priv_op(operation).unwrap();
        }
    }
    for pod_id in pod_ids {
        let pod = all_pods_for_facts.iter().find(|p| p.id() == pod_id);
        if let Some(pod) = pod {
            match pod {
                IndexablePod::SignedPod(pod) => {
                    builder.add_signed_pod(pod);
                }
                IndexablePod::MainPod(pod) => {
                    builder.add_recursive_pod(pod.as_ref().clone());
                }
                IndexablePod::TestPod(_pod) => {}
            }
        }
    }
    let result_main_pod = if mock {
        let prover = MockProver {};
        builder.prove(&prover, &params).unwrap()
    } else {
        let prover = Prover {};
        builder.prove(&prover, &params).unwrap()
    };
    let serialized_pod: SerializedMainPod = result_main_pod.into();

    // Trigger state sync after creating the pod
    app_state.trigger_state_sync().await?;

    Ok(serialized_pod)
}

#[tauri::command]
async fn get_app_state(state: State<'_, Mutex<AppState>>) -> Result<AppStateData, String> {
    let app_state = state.lock().await;
    Ok(app_state.state_data.clone())
}

#[tauri::command]
async fn trigger_sync(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let mut app_state = state.lock().await;
    app_state.trigger_state_sync().await?;
    Ok(())
}

#[tauri::command]
async fn start_p2p_node(state: State<'_, Mutex<AppState>>) -> Result<String, String> {
    let mut app_state = state.lock().await;

    if app_state.p2p_node.is_some() {
        // P2P node already running, just return the NodeID
        let node_id = app_state.p2p_node.as_ref().unwrap().node_id();
        return Ok(node_id.to_string());
    }

    // Create message handler for incoming PODs
    let message_handler =
        p2p::MessageHandler::new(app_state.db.clone(), app_state.app_handle.clone());

    // Spawn new P2P node
    let p2p_node = p2p::P2PNode::spawn(None, Some(message_handler))
        .await
        .map_err(|e| format!("Failed to start P2P node: {}", e))?;

    let node_id = p2p_node.node_id();
    app_state.p2p_node = Some(p2p_node);

    log::info!("P2P node started with NodeID: {}", node_id);
    Ok(node_id.to_string())
}

#[tauri::command]
async fn send_pod_to_peer(
    state: State<'_, Mutex<AppState>>,
    peer_node_id: String,
    pod_id: String,
    message_text: Option<String>,
    sender_alias: Option<String>,
) -> Result<(), String> {
    let app_state = state.lock().await;

    // Ensure P2P node is running
    let p2p_node = app_state
        .p2p_node
        .as_ref()
        .ok_or("P2P node not started. Please start the P2P node first.")?;

    // Get the POD from database
    let pod_info = store::get_pod(&app_state.db, "zukyc", &pod_id)
        .await
        .map_err(|e| format!("Failed to get pod: {}", e))?
        .ok_or("Pod not found")?;

    // Extract SerializedMainPod from PodData
    let serialized_pod = match pod_info.data {
        PodData::Main(main_pod) => main_pod,
        PodData::Signed(_) => {
            return Err("Cannot send SignedPod directly. Only MainPods can be sent.".to_string());
        }
    };

    // Parse peer NodeID
    let peer_id = peer_node_id
        .parse()
        .map_err(|e| format!("Invalid peer node ID: {}", e))?;

    // Send the MainPod
    p2p_node
        .send_main_pod(
            peer_id,
            serialized_pod.clone(),
            message_text.clone(),
            sender_alias,
        )
        .await
        .map_err(|e| format!("Failed to send POD: {}", e))?;

    // Add to chat history
    store::add_sent_message_to_chat(
        &app_state.db,
        &peer_node_id,
        "zukyc",
        &pod_id,
        message_text.as_deref(),
    )
    .await
    .map_err(|e| format!("Failed to record sent message: {}", e))?;

    log::info!("Successfully sent POD {} to peer {}", pod_id, peer_node_id);
    Ok(())
}

#[tauri::command]
async fn get_inbox_messages(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<serde_json::Value>, String> {
    let app_state = state.lock().await;

    store::get_inbox_messages(&app_state.db)
        .await
        .map_err(|e| format!("Failed to get inbox messages: {}", e))
}

#[tauri::command]
async fn accept_inbox_message(
    state: State<'_, Mutex<AppState>>,
    message_id: String,
    chat_alias: Option<String>,
) -> Result<String, String> {
    let mut app_state = state.lock().await;

    let chat_id = store::accept_inbox_message(&app_state.db, &message_id, chat_alias.as_deref())
        .await
        .map_err(|e| format!("Failed to accept inbox message: {}", e))?;

    // Trigger state sync to update frontend
    app_state.trigger_state_sync().await?;

    log::info!(
        "Accepted inbox message {} into chat {}",
        message_id,
        chat_id
    );
    Ok(chat_id)
}

#[tauri::command]
async fn create_private_key(
    state: State<'_, Mutex<AppState>>,
    alias: Option<String>,
    set_as_default: bool,
) -> Result<String, String> {
    let app_state = state.lock().await;

    let key_id = store::create_private_key(&app_state.db, alias.as_deref(), set_as_default)
        .await
        .map_err(|e| format!("Failed to create private key: {}", e))?;

    log::info!(
        "Created private key {} (default: {})",
        key_id,
        set_as_default
    );
    Ok(key_id)
}

#[tauri::command]
async fn list_private_keys(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<serde_json::Value>, String> {
    let app_state = state.lock().await;

    store::list_private_keys(&app_state.db)
        .await
        .map_err(|e| format!("Failed to list private keys: {}", e))
}

#[tauri::command]
async fn send_message_as_pod(
    state: State<'_, Mutex<AppState>>,
    peer_node_id: String,
    message_text: String,
    sender_alias: Option<String>,
) -> Result<(), String> {
    let app_state = state.lock().await;

    // Ensure P2P node is running
    let p2p_node = app_state
        .p2p_node
        .as_ref()
        .ok_or("P2P node not started. Please start the P2P node first.")?;

    // Get default private key
    let private_key = store::get_default_private_key(&app_state.db)
        .await
        .map_err(|e| format!("Failed to get private key: {}", e))?
        .ok_or("No default private key found. Please create a private key first.")?;

    // Create a SignedPod containing the message text
    let params = Params::default();
    let mut builder = SignedPodBuilder::new(&params);

    // Add message content to the POD
    builder.insert("message", PodValue::from(message_text.clone()));
    builder.insert("timestamp", PodValue::from(Utc::now().to_rfc3339()));

    // Create a real Signer using the private key
    let mut signer = Signer(private_key);

    // Sign the POD
    let signed_pod = builder
        .sign(&mut signer)
        .map_err(|e| format!("Failed to sign message POD: {}", e))?;

    // Get pod ID before moving the signed_pod
    let pod_id = signed_pod.id().0.encode_hex::<String>();

    // Store the SignedPod in the database for record keeping
    let pod_data = PodData::Signed(signed_pod.clone().into());
    store::import_pod(&app_state.db, &pod_data, Some("Message POD"), "zukyc")
        .await
        .map_err(|e| format!("Failed to store message POD: {}", e))?;

    // Convert to SerializedSignedPod for P2P transmission
    let serialized_signed_pod: SerializedSignedPod = signed_pod.into();

    // Parse peer NodeID
    let peer_id = peer_node_id
        .parse()
        .map_err(|e| format!("Invalid peer node ID: {}", e))?;

    // Send the SignedPod with message text
    p2p_node
        .send_signed_pod(
            peer_id,
            serialized_signed_pod,
            Some(message_text.clone()),
            sender_alias,
        )
        .await
        .map_err(|e| format!("Failed to send message POD: {}", e))?;

    // Add to chat history
    store::add_sent_message_to_chat(
        &app_state.db,
        &peer_node_id,
        "zukyc",
        &pod_id,
        Some(&message_text),
    )
    .await
    .map_err(|e| format!("Failed to record sent message: {}", e))?;

    log::info!(
        "Successfully sent message POD {} to peer {}",
        pod_id,
        peer_node_id
    );
    Ok(())
}

#[tauri::command]
async fn sign_pod(
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

    // Get default private key
    let private_key = store::get_default_private_key(&app_state.db)
        .await
        .map_err(|e| format!("Failed to get private key: {}", e))?
        .ok_or("No default private key found. Please create a private key first.")?;

    let mut signer = Signer(private_key);

    let signed_pod = builder
        .sign(&mut signer)
        .map_err(|e| format!("Failed to sign pod: {}", e))?;

    Ok(serde_json::to_string(&signed_pod).unwrap())
}

#[tauri::command]
async fn import_pod(
    state: State<'_, Mutex<AppState>>,
    serialized_pod: String,
    pod_type: String,
    label: Option<String>,
) -> Result<(), String> {
    let mut app_state = state.lock().await;

    let pod_data = match pod_type.as_str() {
        "Signed" => PodData::Signed(serde_json::from_str(&serialized_pod).unwrap()),
        "MockSigned" => PodData::Signed(serde_json::from_str(&serialized_pod).unwrap()),
        "Main" => PodData::Main(serde_json::from_str(&serialized_pod).unwrap()),
        "MockMain" => PodData::Main(serde_json::from_str(&serialized_pod).unwrap()),
        _ => return Err(format!("Not a valid POD type: {}", pod_type)),
    };

    let _ = store::import_pod(&app_state.db, &pod_data, label.as_deref(), "zukyc")
        .await
        .map_err(|e| format!("Failed to import POD: {}", e));

    app_state.trigger_state_sync().await?;
    Ok(())
}

#[tauri::command]
async fn get_chats(state: State<'_, Mutex<AppState>>) -> Result<Vec<serde_json::Value>, String> {
    let app_state = state.lock().await;

    store::get_chats(&app_state.db)
        .await
        .map_err(|e| format!("Failed to get chats: {}", e))
}

#[tauri::command]
async fn get_chat_messages(
    state: State<'_, Mutex<AppState>>,
    chat_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let app_state = state.lock().await;

    store::get_chat_messages(&app_state.db, &chat_id)
        .await
        .map_err(|e| format!("Failed to get chat messages: {}", e))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateData {
    pub pod_stats: PodStats,
    pub pod_lists: PodLists,
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
        // For now, we're using "zukyc" as the default space
        // This could be made configurable in the future
        let space_id = "zukyc";

        let signed_pods = store::list_pods_by_type(&self.db, space_id, "signed")
            .await
            .map_err(|e| format!("Failed to list signed pods: {}", e))?;

        let main_pods = store::list_pods_by_type(&self.db, space_id, "main")
            .await
            .map_err(|e| format!("Failed to list main pods: {}", e))?;

        self.state_data.pod_lists = PodLists {
            signed_pods,
            main_pods,
        };

        Ok(())
    }

    pub async fn trigger_state_sync(&mut self) -> Result<(), String> {
        // This can be called from anywhere to refresh all state
        self.refresh_pod_stats().await?;
        self.refresh_pod_lists().await?;
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

pub async fn setup_zukyc_space(db: &Db) -> anyhow::Result<()> {
    let space_id = "zukyc";

    if store::space_exists(db, space_id).await? {
        log::info!("Space '{}' already exists. Skipping setup.", space_id);
        return Ok(());
    }

    log::info!("Setting up space '{}' with Zukyc sample pods...", space_id);
    store::create_space(db, space_id).await?;

    match sign_zukyc_pods() {
        Ok(pods) => {
            log::info!("All pods signed successfully, importing to DB...");
            let pod_names = ["Gov ID", "Pay Stub", "Sanctions List"];

            for (pod, name) in pods.into_iter().zip(pod_names) {
                let pod_data = PodData::from(pod);
                store::import_pod(db, &pod_data, Some(name), space_id).await?;
            }
            log::info!("Successfully set up Zukyc space.");
        }
        Err(e) => {
            log::error!("Failed to sign one or more pods for Zukyc setup: {}", e);
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

    setup_zukyc_space(&db).await?;

    Ok(db)
}

fn set_default_config(app: &mut App, store_name: &str) {
    let store = app.store(store_name).unwrap();

    if store.get("instance_id").is_none() {
        store.set("instance_id", "default");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();
    let mut builder = tauri::Builder::default()
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
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            submit_pod_request,
            get_app_state,
            trigger_sync,
            start_p2p_node,
            send_pod_to_peer,
            get_inbox_messages,
            accept_inbox_message,
            create_private_key,
            list_private_keys,
            send_message_as_pod,
            get_chats,
            get_chat_messages,
            sign_pod,
            import_pod
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
