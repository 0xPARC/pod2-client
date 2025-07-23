use anyhow::Result;
use chrono::{DateTime, Utc};
use pod2::{
    backends::plonky2::{
        primitives::ec::{curve::Point as PublicKey, schnorr::SecretKey},
        signedpod::Signer,
    },
    frontend::{SignedPod, SignedPodBuilder},
    middleware::Params,
};
use pod_utils::ValueExt;
use serde::{Deserialize, Serialize};

use crate::github::GitHubUser;

#[derive(Debug, Serialize)]
pub struct IdentityResponse {
    pub identity_pod: SignedPod,
}

#[derive(Debug, Serialize)]
pub struct ServerInfo {
    pub server_id: String,
    pub public_key: PublicKey,
}

#[derive(Debug, Deserialize)]
pub struct UsernameLookupRequest {
    pub public_key: PublicKey,
}

#[derive(Debug, Serialize)]
pub struct UsernameLookupResponse {
    pub username: String,
}

pub fn create_identity_pod(
    server_id: &str,
    server_secret_key: &SecretKey,
    public_key: &PublicKey,
    username: &str,
    github_user: &GitHubUser,
    github_public_keys: &[String],
    oauth_verified_at: DateTime<Utc>,
) -> Result<SignedPod> {
    let params = Params::default();
    let mut identity_builder = SignedPodBuilder::new(&params);

    // Core identity fields (minimal in main pod)
    identity_builder.insert("username", username);
    identity_builder.insert("user_public_key", *public_key);
    identity_builder.insert("identity_server_id", server_id);
    identity_builder.insert("issued_at", Utc::now().to_rfc3339().as_str());

    // Create GitHub data dictionary (similar to document pod structure)
    let mut github_data = std::collections::HashMap::new();
    github_data.insert("github_username".to_string(), serde_json::Value::String(github_user.login.clone()));
    github_data.insert("github_user_id".to_string(), serde_json::Value::Number(github_user.id.into()));
    github_data.insert("oauth_verified_at".to_string(), serde_json::Value::String(oauth_verified_at.to_rfc3339()));
    github_data.insert("github_public_keys".to_string(), serde_json::Value::Array(
        github_public_keys.iter().map(|k| serde_json::Value::String(k.clone())).collect()
    ));
    
    // Add email if available
    if let Some(email) = &github_user.email {
        github_data.insert("github_email".to_string(), serde_json::Value::String(email.clone()));
    }

    // Store GitHub data as a dictionary field
    let github_data_json = serde_json::to_string(&github_data)?;
    identity_builder.insert("github_data", github_data_json.as_str());

    // Sign the identity pod with the identity server's key
    let server_signer = Signer(SecretKey(server_secret_key.0.clone()));
    let identity_pod = identity_builder.sign(&server_signer)?;

    tracing::info!(
        "Identity pod issued for user: {} (GitHub: {})",
        username,
        github_user.login
    );

    Ok(identity_pod)
}

pub fn verify_identity_pod(identity_pod: &SignedPod) -> Result<()> {
    // Verify the signature
    identity_pod.verify()?;

    // Verify required fields are present in main pod
    let required_fields = [
        "username",
        "user_public_key",
        "identity_server_id",
        "issued_at",
        "github_data",
        "_signer",
    ];

    for field in &required_fields {
        if identity_pod.get(*field).is_none() {
            return Err(anyhow::anyhow!("Missing required field: {}", field));
        }
    }

    // Parse and verify GitHub data dictionary
    let github_data_str = identity_pod
        .get("github_data")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid github_data field"))?;
    
    let github_data: std::collections::HashMap<String, serde_json::Value> = 
        serde_json::from_str(github_data_str)?;

    // Verify required GitHub data fields
    let required_github_fields = [
        "github_username",
        "github_user_id", 
        "oauth_verified_at",
        "github_public_keys",
    ];

    for field in &required_github_fields {
        if !github_data.contains_key(*field) {
            return Err(anyhow::anyhow!("Missing required GitHub data field: {}", field));
        }
    }

    // Verify the OAuth timestamp is not in the future
    let oauth_verified_at_str = github_data
        .get("oauth_verified_at")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid oauth_verified_at field in GitHub data"))?;
    
    let oauth_verified_at = DateTime::parse_from_rfc3339(oauth_verified_at_str)?;
    if oauth_verified_at > Utc::now() {
        return Err(anyhow::anyhow!("OAuth verification timestamp is in the future"));
    }

    tracing::info!("✓ Identity pod verification passed");
    Ok(())
}

pub fn extract_identity_info(identity_pod: &SignedPod) -> Result<(String, String, PublicKey, i64)> {
    let username = identity_pod
        .get("username")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing username"))?;

    let user_public_key = identity_pod
        .get("user_public_key")
        .and_then(|v| v.as_public_key())
        .ok_or_else(|| anyhow::anyhow!("Missing user_public_key"))?;

    // Parse GitHub data dictionary
    let github_data_str = identity_pod
        .get("github_data")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing github_data"))?;
    
    let github_data: std::collections::HashMap<String, serde_json::Value> = 
        serde_json::from_str(github_data_str)?;

    let github_username = github_data
        .get("github_username")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing github_username in GitHub data"))?;

    let github_user_id = github_data
        .get("github_user_id")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| anyhow::anyhow!("Missing github_user_id in GitHub data"))?;

    Ok((
        username.to_string(),
        github_username.to_string(),
        *user_public_key,
        github_user_id,
    ))
}