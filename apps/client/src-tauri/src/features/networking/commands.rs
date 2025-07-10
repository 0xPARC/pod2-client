use crate::{AppState, DEFAULT_SPACE_ID, p2p};
use pod2_db::store::{self, PodData};
use pod2::{
    backends::plonky2::signedpod::Signer,
    frontend::{SignedPodBuilder, SerializedSignedPod},
    middleware::{Params, Value as PodValue},
};
use chrono::Utc;
use hex::ToHex;
use tauri::{State, AppHandle};
use tokio::sync::Mutex;

/// Start the P2P node
#[tauri::command]
pub async fn start_p2p_node(state: State<'_, Mutex<AppState>>) -> Result<String, String> {
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

/// Send a POD to a peer
#[tauri::command]
pub async fn send_pod_to_peer(
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
    let pod_info = store::get_pod(&app_state.db, DEFAULT_SPACE_ID, &pod_id)
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
        DEFAULT_SPACE_ID,
        &pod_id,
        message_text.as_deref(),
    )
    .await
    .map_err(|e| format!("Failed to record sent message: {}", e))?;

    log::info!("Successfully sent POD {} to peer {}", pod_id, peer_node_id);
    Ok(())
}

/// Send a message as a POD to a peer
#[tauri::command]
pub async fn send_message_as_pod(
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

    // Get default private key (auto-created if needed)
    let private_key = store::get_default_private_key(&app_state.db)
        .await
        .map_err(|e| format!("Failed to get private key: {}", e))?;

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
    store::import_pod(
        &app_state.db,
        &pod_data,
        Some("Message POD"),
        DEFAULT_SPACE_ID,
    )
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
        DEFAULT_SPACE_ID,
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

/// Get inbox messages
#[tauri::command]
pub async fn get_inbox_messages(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<serde_json::Value>, String> {
    let app_state = state.lock().await;

    store::get_inbox_messages(&app_state.db)
        .await
        .map_err(|e| format!("Failed to get inbox messages: {}", e))
}

/// Accept an inbox message
#[tauri::command]
pub async fn accept_inbox_message(
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

/// Get all chats
#[tauri::command]
pub async fn get_chats(state: State<'_, Mutex<AppState>>) -> Result<Vec<serde_json::Value>, String> {
    let app_state = state.lock().await;

    store::get_chats(&app_state.db)
        .await
        .map_err(|e| format!("Failed to get chats: {}", e))
}

/// Get messages for a specific chat
#[tauri::command]
pub async fn get_chat_messages(
    state: State<'_, Mutex<AppState>>,
    chat_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let app_state = state.lock().await;

    store::get_chat_messages(&app_state.db, &chat_id)
        .await
        .map_err(|e| format!("Failed to get chat messages: {}", e))
}

/// Generate handler for networking commands
pub fn networking_commands() -> impl Fn(tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    |builder| {
        builder.invoke_handler(tauri::generate_handler![
            start_p2p_node,
            send_pod_to_peer,
            send_message_as_pod,
            get_inbox_messages,
            accept_inbox_message,
            get_chats,
            get_chat_messages
        ])
    }
}