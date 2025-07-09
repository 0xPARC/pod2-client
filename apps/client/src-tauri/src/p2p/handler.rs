use anyhow::Result;
use chrono::Utc;
use iroh::NodeId;
use pod2_db::{store, Db};
use tauri::{AppHandle, Emitter};
use tracing::{error, info};

use super::message::{PodMessage, ReceivedMessage, SignedPodMessage};

#[derive(Clone)]
pub struct MessageHandler {
    db: Db,
    app_handle: AppHandle,
}

impl std::fmt::Debug for MessageHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageHandler")
            .field("db", &"<Db>")
            .field("app_handle", &"<AppHandle>")
            .finish()
    }
}

impl MessageHandler {
    pub fn new(db: Db, app_handle: AppHandle) -> Self {
        Self { db, app_handle }
    }

    /// Handle a received raw message from a peer
    pub async fn handle_received_message(&self, from: NodeId, bytes: Vec<u8>) -> Result<()> {
        info!(
            "Processing received message: {} bytes from {}",
            bytes.len(),
            from
        );

        // Verify and decode the message
        let (sender_node_id, pod_message) = match SignedPodMessage::verify_and_decode(&bytes) {
            Ok(result) => {
                info!(
                    "Successfully verified and decoded message from {}",
                    result.0
                );
                result
            }
            Err(e) => {
                error!("Failed to verify/decode message from {}: {}", from, e);
                return Err(e);
            }
        };

        // Create received message struct
        let received_message = ReceivedMessage {
            from: sender_node_id,
            message: pod_message,
            timestamp: Utc::now(),
        };

        // Process the message based on type
        match &received_message.message {
            PodMessage::SendMainPod {
                pod,
                message_text,
                sender_alias,
            } => {
                self.handle_send_main_pod(
                    received_message.from,
                    pod.clone(),
                    message_text.clone(),
                    sender_alias.clone(),
                )
                .await?;
            }
            PodMessage::SendSignedPod {
                pod,
                message_text,
                sender_alias,
            } => {
                self.handle_send_signed_pod(
                    received_message.from,
                    pod.clone(),
                    message_text.clone(),
                    sender_alias.clone(),
                )
                .await?;
            }
        }

        info!(
            "Successfully processed message from {}",
            received_message.from
        );
        Ok(())
    }

    /// Handle a SendMainPod message by importing the POD and storing in inbox
    async fn handle_send_main_pod(
        &self,
        from_node_id: NodeId,
        pod: pod2::frontend::SerializedMainPod,
        message_text: Option<String>,
        sender_alias: Option<String>,
    ) -> Result<()> {
        // Convert to PodData for import
        let main_pod = pod2::frontend::MainPod::try_from(pod)?;
        let pod_data = pod2_db::store::PodData::Main(main_pod.into());

        // Extract message text from POD if not provided
        let final_message_text = message_text.or({
            // Try to extract POD["message"] entry as string
            // This is a simplified extraction - in practice we'd need to properly
            // traverse the POD structure to find a "message" key
            None // TODO: Implement POD message extraction
        });

        // Import POD and add to inbox in a single transaction to avoid foreign key issues
        let message_id = store::import_pod_and_add_to_inbox(
            &self.db,
            &pod_data,
            "zukyc",
            &from_node_id.to_string(),
            sender_alias.as_deref(),
            final_message_text.as_deref(),
        )
        .await?;

        let pod_id = pod_data.id();

        info!(
            "Added message {} to inbox from {}",
            message_id, from_node_id
        );

        // Emit event to frontend
        self.app_handle.emit(
            "p2p-pod-received",
            serde_json::json!({
                "message_id": message_id,
                "from_node_id": from_node_id.to_string(),
                "from_alias": sender_alias,
                "pod_id": pod_id,
                "message_text": final_message_text,
                "received_at": Utc::now().to_rfc3339()
            }),
        )?;

        Ok(())
    }

    /// Handle a SendSignedPod message by importing the POD and storing in inbox
    async fn handle_send_signed_pod(
        &self,
        from_node_id: NodeId,
        pod: pod2::frontend::SerializedSignedPod,
        message_text: Option<String>,
        sender_alias: Option<String>,
    ) -> Result<()> {
        // Convert to PodData for import
        let signed_pod = pod2::frontend::SignedPod::try_from(pod)?;
        let pod_data = pod2_db::store::PodData::Signed(signed_pod.clone().into());

        // Use the provided message text
        // TODO: Implement POD value extraction when needed
        let final_message_text = message_text;

        // Import POD and add to inbox in a single transaction to avoid foreign key issues
        let message_id = store::import_pod_and_add_to_inbox(
            &self.db,
            &pod_data,
            "zukyc",
            &from_node_id.to_string(),
            sender_alias.as_deref(),
            final_message_text.as_deref(),
        )
        .await?;

        let pod_id = pod_data.id();

        info!(
            "Added SignedPod message {} to inbox from {}",
            message_id, from_node_id
        );

        // Emit event to frontend
        self.app_handle.emit(
            "p2p-pod-received",
            serde_json::json!({
                "message_id": message_id,
                "from_node_id": from_node_id.to_string(),
                "from_alias": sender_alias,
                "pod_id": pod_id,
                "message_text": final_message_text,
                "received_at": Utc::now().to_rfc3339()
            }),
        )?;

        Ok(())
    }
}
