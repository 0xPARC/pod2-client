use anyhow::Result;
use iroh::{
    endpoint::{Connection, RemoteInfo},
    protocol::{ProtocolHandler, Router},
    Endpoint, NodeId, SecretKey,
};
use n0_future::boxed::BoxFuture;
use tracing::{error, info, warn};

use super::{handler::MessageHandler, message::SignedPodMessage};

const P2P_ALPN: &[u8] = b"pod2/p2p/1";

pub struct P2PNode {
    secret_key: SecretKey,
    router: Router,
}

/// Protocol handler for POD messages
#[derive(Debug, Clone)]
pub struct PodProtocolHandler {
    handler: MessageHandler,
}

impl PodProtocolHandler {
    pub fn new(handler: MessageHandler) -> Self {
        Self { handler }
    }
}

impl ProtocolHandler for PodProtocolHandler {
    fn accept(&self, connection: Connection) -> BoxFuture<anyhow::Result<()>> {
        let handler = self.handler.clone();
        Box::pin(async move {
            let node_id = connection.remote_node_id()?;
            info!("Accepted POD connection from {}", node_id);

            // Accept a bidirectional stream
            let (mut send, mut recv) = connection.accept_bi().await?;

            // Read the message
            let message_bytes = recv.read_to_end(1024 * 1024).await?;
            info!("Received {} bytes from {}", message_bytes.len(), node_id);

            // Process the message
            match handler
                .handle_received_message(node_id, message_bytes)
                .await
            {
                Ok(_) => {
                    info!("Successfully processed message from {}", node_id);
                    // Send acknowledgment
                    send.write_all(b"ACK").await?;
                }
                Err(e) => {
                    error!("Failed to handle message from {}: {}", node_id, e);
                    // Send error response
                    send.write_all(b"ERR").await?;
                }
            }

            // Finish the send stream
            send.finish()?;

            // Wait for connection to close
            connection.closed().await;

            Ok(())
        })
    }
}

impl P2PNode {
    /// Spawns a P2P node for direct POD messaging using Iroh Router pattern
    pub async fn spawn(
        secret_key: Option<SecretKey>,
        message_handler: Option<MessageHandler>,
    ) -> Result<Self> {
        let secret_key = secret_key.unwrap_or_else(|| SecretKey::generate(rand::rngs::OsRng));

        let endpoint = Endpoint::builder()
            .secret_key(secret_key.clone())
            .discovery_n0()
            .alpns(vec![P2P_ALPN.to_vec()])
            .bind()
            .await?;

        let node_id = endpoint.node_id();
        info!("P2P endpoint bound");
        info!("P2P node id: {node_id:#?}");

        // Create router with protocol handler if provided
        let router = if let Some(handler) = message_handler {
            let protocol_handler = PodProtocolHandler::new(handler);
            Router::builder(endpoint)
                .accept(P2P_ALPN, protocol_handler)
                .spawn()
        } else {
            // Just create router without accepting any protocols
            Router::builder(endpoint).spawn()
        };

        info!("P2P router spawned");

        Ok(Self { secret_key, router })
    }

    /// Returns the node id of this node
    pub fn node_id(&self) -> NodeId {
        self.router.endpoint().node_id()
    }

    /// Send a MainPod to a specific peer with optional message
    pub async fn send_main_pod(
        &self,
        recipient_node_id: NodeId,
        pod: pod2::frontend::SerializedMainPod,
        message_text: Option<String>,
        sender_alias: Option<String>,
    ) -> Result<()> {
        let message = super::message::PodMessage::SendMainPod {
            pod: Box::new(pod),
            message_text,
            sender_alias,
        };

        self.send_message_to_peer(recipient_node_id, message).await
    }

    /// Send a SignedPod to a specific peer with optional message
    pub async fn send_signed_pod(
        &self,
        recipient_node_id: NodeId,
        pod: pod2::frontend::SerializedSignedPod,
        message_text: Option<String>,
        sender_alias: Option<String>,
    ) -> Result<()> {
        let message = super::message::PodMessage::SendSignedPod {
            pod: Box::new(pod),
            message_text,
            sender_alias,
        };

        self.send_message_to_peer(recipient_node_id, message).await
    }

    /// Send a signed POD message to a specific peer
    pub async fn send_message_to_peer(
        &self,
        recipient_node_id: NodeId,
        message: super::message::PodMessage,
    ) -> Result<()> {
        info!("Attempting to send message to peer: {}", recipient_node_id);

        // Sign and encode the message
        let signed_message_bytes = SignedPodMessage::sign_and_encode(&self.secret_key, message)?;
        info!(
            "Message signed and encoded, {} bytes",
            signed_message_bytes.len()
        );

        // Connect to the peer using the router endpoint
        info!("Connecting to peer: {}", recipient_node_id);
        let connection = self
            .router
            .endpoint()
            .connect(recipient_node_id, P2P_ALPN)
            .await
            .map_err(|e| {
                error!("Failed to connect to peer {}: {}", recipient_node_id, e);
                e
            })?;
        info!("Successfully connected to peer: {}", recipient_node_id);

        // Open a bidirectional stream like in iroh-ping example
        let (mut send, mut recv) = connection.open_bi().await?;

        // Send the message
        send.write_all(&signed_message_bytes).await?;
        send.finish()?;

        // Wait for acknowledgment
        let response = recv.read_to_end(4).await?;
        match &response[..] {
            b"ACK" => info!("Received acknowledgment from peer"),
            b"ERR" => warn!("Peer reported error processing message"),
            _ => warn!("Received unexpected response: {:?}", response),
        }

        // Close connection
        connection.close(0u32.into(), b"done");

        info!(
            "Successfully sent POD message to peer: {}",
            recipient_node_id
        );
        Ok(())
    }

    /// Get information about remote nodes
    #[allow(unused)]
    pub fn remote_info(&self) -> Vec<RemoteInfo> {
        self.router
            .endpoint()
            .remote_info_iter()
            .collect::<Vec<_>>()
    }

    /// Shutdown the P2P node
    #[allow(unused)]
    pub async fn shutdown(&self) {
        if let Err(err) = self.router.shutdown().await {
            warn!("Failed to shutdown router cleanly: {}", err);
        }
        self.router.endpoint().close().await;
    }
}
