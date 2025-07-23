use anyhow::Result;
use pod2::{
    backends::plonky2::{
        primitives::ec::{curve::Point as PublicKey, schnorr::SecretKey},
        signedpod::Signer,
    },
    frontend::{SignedPod, SignedPodBuilder},
    middleware::Params,
};
use pod_utils::ValueExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};

// Registration models for registering with podnet-server
#[derive(Debug, Serialize)]
pub struct IdentityServerChallengeRequest {
    pub server_id: String,
    pub public_key: PublicKey,
}

#[derive(Debug, Deserialize)]
pub struct IdentityServerChallengeResponse {
    pub challenge_pod: SignedPod,
}

#[derive(Debug, Serialize)]
pub struct IdentityServerRegistrationRequest {
    pub server_challenge_pod: SignedPod,
    pub identity_response_pod: SignedPod,
}

#[derive(Debug, Deserialize)]
pub struct PodNetServerInfo {
    pub public_key: PublicKey,
}

pub async fn register_with_podnet_server(
    server_id: &str,
    secret_key: &SecretKey,
    podnet_server_url: &str,
) -> Result<()> {
    tracing::info!("Registering GitHub identity server with podnet-server at: {}", podnet_server_url);

    let public_key = secret_key.public_key();
    let client = Client::new();

    // Step 1: Request challenge from server
    tracing::info!("Requesting challenge from podnet-server");
    let challenge_request = IdentityServerChallengeRequest {
        server_id: server_id.to_string(),
        public_key,
    };

    let challenge_response = client
        .post(format!("{podnet_server_url}/identity/challenge"))
        .header("Content-Type", "application/json")
        .json(&challenge_request)
        .send()
        .await?;

    if !challenge_response.status().is_success() {
        let status = challenge_response.status();
        let error_text = challenge_response.text().await?;
        return Err(anyhow::anyhow!(
            "Failed to get challenge. Status: {status} - {error_text}"
        ));
    }

    let challenge_response: IdentityServerChallengeResponse = challenge_response.json().await?;
    tracing::info!("✓ Received challenge from podnet-server");

    // Step 2: Verify the server's challenge pod
    challenge_response.challenge_pod.verify()?;
    tracing::info!("✓ Verified server's challenge pod signature");

    // Extract challenge from server's pod
    let challenge = challenge_response
        .challenge_pod
        .get("challenge")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Server challenge pod missing challenge"))?;

    tracing::info!("Challenge received: {}", challenge);

    // Step 3: Create identity server's response pod
    let params = Params::default();
    let mut response_builder = SignedPodBuilder::new(&params);

    response_builder.insert("challenge", challenge);
    response_builder.insert("server_id", server_id);
    response_builder.insert("server_type", "github-oauth");

    // Sign the response with identity server's private key
    let identity_signer = Signer(SecretKey(secret_key.0.clone()));
    let identity_response_pod = response_builder.sign(&identity_signer)?;

    tracing::info!("✓ Created GitHub identity server response pod");

    // Step 4: Submit both pods for registration
    let registration_request = IdentityServerRegistrationRequest {
        server_challenge_pod: challenge_response.challenge_pod,
        identity_response_pod,
    };

    let registration_response = client
        .post(format!("{podnet_server_url}/identity/register"))
        .header("Content-Type", "application/json")
        .json(&registration_request)
        .send()
        .await?;

    if registration_response.status().is_success() {
        let server_info: PodNetServerInfo = registration_response.json().await?;
        tracing::info!("✓ Successfully registered GitHub identity server with podnet-server!");
        tracing::info!("PodNet Server Public Key: {}", server_info.public_key);
        Ok(())
    } else {
        let status = registration_response.status();
        let error_text = registration_response.text().await?;

        if status == reqwest::StatusCode::CONFLICT {
            tracing::info!("✓ GitHub identity server already registered with podnet-server");
            Ok(())
        } else {
            tracing::error!("Failed to register with podnet-server. Status: {}", status);
            tracing::error!("Error: {}", error_text);
            Err(anyhow::anyhow!("Registration failed: {status} - {error_text}"))
        }
    }
}