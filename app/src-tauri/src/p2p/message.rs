use anyhow::Result;
use iroh::{NodeId, PublicKey, SecretKey};
use iroh_base::Signature;
use pod2::frontend::{SerializedMainPod, SerializedSignedPod};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PodMessage {
    SendMainPod {
        pod: SerializedMainPod,
        message_text: Option<String>,
        sender_alias: Option<String>,
    },
    SendSignedPod {
        pod: SerializedSignedPod,
        message_text: Option<String>,
        sender_alias: Option<String>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SignedPodMessage {
    from: PublicKey,
    data: Vec<u8>, // JSON-serialized PodMessage
    signature: Signature,
}

impl SignedPodMessage {
    /// Sign and encode a POD message to bytes using JSON serialization
    pub fn sign_and_encode(secret_key: &SecretKey, message: PodMessage) -> Result<Vec<u8>> {
        // Serialize the message to JSON
        let data = serde_json::to_vec(&message)?;
        
        // Sign the data
        let signature = secret_key.sign(&data);
        
        // Create signed message
        let signed_message = Self {
            from: secret_key.public(),
            data,
            signature,
        };
        
        // Serialize the entire signed message to JSON
        let encoded = serde_json::to_vec(&signed_message)?;
        Ok(encoded)
    }

    /// Verify signature and decode a received POD message
    pub fn verify_and_decode(bytes: &[u8]) -> Result<(NodeId, PodMessage)> {
        // Deserialize the signed message from JSON
        let signed_message: Self = serde_json::from_slice(bytes)?;
        
        // Verify the signature
        let key: PublicKey = signed_message.from;
        key.verify(&signed_message.data, &signed_message.signature)?;
        
        // Deserialize the inner message
        let message: PodMessage = serde_json::from_slice(&signed_message.data)?;
        
        Ok((signed_message.from, message))
    }
}

/// Represents a received message from a peer
#[derive(Debug, Clone)]
pub struct ReceivedMessage {
    pub from: NodeId,
    pub message: PodMessage,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}