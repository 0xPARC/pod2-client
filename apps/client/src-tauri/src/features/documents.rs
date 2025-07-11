use std::collections::HashMap;

use podnet_models::Document;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentVerificationResult {
    pub publish_verified: bool,
    pub timestamp_verified: bool,
    pub upvote_count_verified: bool,
    pub verification_details: HashMap<String, String>,
    pub verification_errors: Vec<String>,
}

#[tauri::command]
pub async fn verify_document_pod(document: Document) -> Result<DocumentVerificationResult, String> {
    let mut verification_result = DocumentVerificationResult {
        publish_verified: false,
        timestamp_verified: false,
        upvote_count_verified: false,
        verification_details: HashMap::new(),
        verification_errors: Vec::new(),
    };

    // Get server public key - for now use a placeholder
    // TODO: This should be configurable or fetched from the server
    let server_public_key = "your_server_public_key_here";

    // Use the simplified Document.verify() method
    match document.verify(server_public_key) {
        Ok(()) => {
            // All verification checks passed
            verification_result.publish_verified = true;
            verification_result.timestamp_verified = true;
            verification_result.upvote_count_verified = true;

            verification_result.verification_details.insert(
                "publish_verification".to_string(),
                "Identity, document, and content hash verification passed".to_string(),
            );
            verification_result.verification_details.insert(
                "timestamp_verification".to_string(),
                "Server timestamp signature verified".to_string(),
            );
            verification_result.verification_details.insert(
                "upvote_count_verification".to_string(),
                "Upvote count cryptographic proof verified".to_string(),
            );
        }
        Err(e) => {
            verification_result
                .verification_errors
                .push(format!("Document verification failed: {}", e));
        }
    }

    Ok(verification_result)
}
