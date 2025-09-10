use std::fs::File;

use pod_utils::ValueExt;
use pod2::{
    backends::plonky2::{primitives::ec::curve::Point as PublicKey, signer::Signer},
    frontend::{SignedDict, SignedDictBuilder},
    middleware::Params,
};
use serde::{Deserialize, Serialize};

use crate::{commands::keygen::KeypairData, utils::handle_error_response};

#[derive(Debug, Deserialize)]
pub struct ChallengeResponse {
    pub challenge_pod: SignedDict,
}

#[derive(Debug, Serialize)]
pub struct ChallengeRequest {
    pub username: String,
    pub user_public_key: PublicKey,
}

#[derive(Debug, Deserialize)]
pub struct IdentityResponse {
    pub identity_pod: SignedDict,
}

#[derive(Debug, Serialize)]
pub struct IdentityRequest {
    pub server_challenge_pod: SignedDict,
    pub user_response_pod: SignedDict,
}

// GitHub Identity Server structures
#[derive(Debug, Deserialize)]
pub struct ServerInfo {
    pub server_id: String,
    pub _public_key: PublicKey,
}

#[derive(Debug, Serialize)]
pub struct GitHubAuthUrlRequest {
    pub public_key: PublicKey,
    pub username: String,
}

#[derive(Debug, Deserialize)]
pub struct GitHubAuthUrlResponse {
    pub auth_url: String,
    pub state: String,
}

#[derive(Debug, Serialize)]
pub struct GitHubIdentityRequest {
    pub code: String,
    pub state: String,
    pub username: String,
    pub challenge_signature: String, // For now, we'll use an empty string
}

#[derive(Debug)]
pub enum IdentityServerType {
    Standard,
    GitHub,
}

async fn detect_identity_server_type(
    client: &reqwest::Client,
    identity_server_url: &str,
) -> Result<IdentityServerType, Box<dyn std::error::Error>> {
    println!("Detecting identity server type...");

    // Try to get server info from root endpoint
    let response = client.get(format!("{identity_server_url}/")).send().await?;

    if !response.status().is_success() {
        return Err("Failed to connect to identity server".into());
    }

    let server_info: ServerInfo = response.json().await?;

    if server_info.server_id.contains("github") {
        println!("✓ Detected GitHub Identity Server");
        Ok(IdentityServerType::GitHub)
    } else {
        println!("✓ Detected Standard Identity Server");
        Ok(IdentityServerType::Standard)
    }
}

async fn get_github_identity(
    client: &reqwest::Client,
    identity_server_url: &str,
    username: &str,
    public_key: PublicKey,
    output_file: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting GitHub OAuth flow...");

    // Step 1: Get GitHub OAuth authorization URL
    let auth_request = GitHubAuthUrlRequest {
        public_key,
        username: username.to_string(),
    };

    let auth_response = client
        .post(format!("{identity_server_url}/auth/github"))
        .header("Content-Type", "application/json")
        .json(&auth_request)
        .send()
        .await?;

    if !auth_response.status().is_success() {
        let status = auth_response.status();
        let error_text = auth_response.text().await?;
        handle_error_response(status, &error_text, "get GitHub auth URL");
        return Ok(());
    }

    let auth_data: GitHubAuthUrlResponse = auth_response.json().await?;

    println!("✓ GitHub OAuth URL generated");
    println!("Please visit the following URL to authorize with GitHub:");
    println!();
    println!("{}", auth_data.auth_url);
    println!();
    println!("After authorizing, you will be redirected to a page with an authorization code.");
    println!("Please copy the authorization code and paste it here:");

    // Read authorization code from user input
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let auth_code = input.trim().to_string();

    if auth_code.is_empty() {
        return Err("Authorization code is required".into());
    }

    println!("✓ Authorization code received");

    // Step 2: Complete identity verification
    let identity_request = GitHubIdentityRequest {
        code: auth_code,
        state: auth_data.state,
        username: username.to_string(),
        challenge_signature: String::new(), // Empty for now - server doesn't validate this yet
    };

    let identity_response = client
        .post(format!("{identity_server_url}/identity"))
        .header("Content-Type", "application/json")
        .json(&identity_request)
        .send()
        .await?;

    if !identity_response.status().is_success() {
        let status = identity_response.status();
        let error_text = identity_response.text().await?;
        handle_error_response(status, &error_text, "complete GitHub identity verification");
        return Ok(());
    }

    let identity_data: IdentityResponse = identity_response.json().await?;

    // Verify the identity pod
    identity_data.identity_pod.verify()?;
    println!("✓ GitHub identity pod verification successful");

    // Save identity pod to file
    let identity_json = serde_json::to_string_pretty(&identity_data.identity_pod)?;
    std::fs::write(output_file, identity_json)?;

    println!("✓ GitHub identity pod saved to: {output_file}");
    println!("✓ GitHub identity acquired successfully!");
    println!("Username: {username}");

    Ok(())
}

async fn get_standard_identity(
    client: &reqwest::Client,
    identity_server_url: &str,
    username: &str,
    keypair_data: &KeypairData,
    output_file: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Using standard identity server flow...");

    let public_key = keypair_data.public_key;

    // Step 1: Request challenge from identity server
    println!("Requesting challenge from identity server...");

    let challenge_request = ChallengeRequest {
        username: username.to_string(),
        user_public_key: public_key,
    };

    let challenge_response = client
        .post(format!("{identity_server_url}/user/challenge"))
        .header("Content-Type", "application/json")
        .json(&challenge_request)
        .send()
        .await?;

    if !challenge_response.status().is_success() {
        let status = challenge_response.status();
        let error_text = challenge_response.text().await?;
        handle_error_response(status, &error_text, "request challenge");
        return Ok(());
    }

    let challenge_data: ChallengeResponse = challenge_response.json().await?;

    // Verify the challenge pod signature
    challenge_data.challenge_pod.verify()?;
    println!("✓ Received and verified challenge pod from identity server");

    // Extract challenge from the signed pod
    let challenge = challenge_data
        .challenge_pod
        .get("challenge")
        .and_then(|v| v.as_str())
        .ok_or("Challenge pod missing challenge field")?;

    println!("Challenge: {challenge}");

    // Step 2: Sign the challenge and send back to get identity pod
    println!("Signing challenge response...");

    // Parse secret key from hex
    let secret_key_bytes = hex::decode(&keypair_data.secret_key)?;
    let secret_key_bigint = num_bigint::BigUint::from_bytes_le(&secret_key_bytes);
    let secret_key = pod2::backends::plonky2::primitives::ec::schnorr::SecretKey(secret_key_bigint);

    // Create challenge response pod
    let params = Params::default();
    let mut challenge_builder = SignedDictBuilder::new(&params);

    challenge_builder.insert("challenge", challenge);
    challenge_builder.insert("username", username);

    // Sign the challenge response
    let user_signer = Signer(secret_key);
    let challenge_response_pod = challenge_builder.sign(&user_signer)?;

    // Extract server_id from challenge pod before moving it
    let server_id = challenge_data
        .challenge_pod
        .get("server_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let identity_request = IdentityRequest {
        server_challenge_pod: challenge_data.challenge_pod,
        user_response_pod: challenge_response_pod,
    };

    // Step 3: Submit signed challenge to get identity pod
    println!("Submitting signed challenge to get identity pod...");
    println!("Server challenge pod verified: ✓");
    println!("User response pod created: ✓");

    let identity_response = client
        .post(format!("{identity_server_url}/identity"))
        .header("Content-Type", "application/json")
        .json(&identity_request)
        .send()
        .await?;

    if !identity_response.status().is_success() {
        let status = identity_response.status();
        let error_text = identity_response.text().await?;
        handle_error_response(status, &error_text, "get identity pod");
        return Ok(());
    }

    let identity_data: IdentityResponse = identity_response.json().await?;

    // Verify the identity pod
    identity_data.identity_pod.verify()?;
    println!("✓ Identity pod verification successful");

    // Save identity pod to file
    let identity_json = serde_json::to_string_pretty(&identity_data.identity_pod)?;
    std::fs::write(output_file, identity_json)?;

    println!("✓ Identity pod saved to: {output_file}");
    println!("✓ Identity acquired successfully!");
    println!("Username: {username}");

    // Display server_id if available
    if let Some(server_id) = server_id {
        println!("Identity Server: {server_id}");
    }

    Ok(())
}

pub async fn get_identity(
    keypair_file: &str,
    identity_server_url: &str,
    username: &str,
    output_file: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Getting identity from identity server...");
    println!("Username: {username}");
    println!("Identity Server: {identity_server_url}");

    // Load keypair from file
    let file = File::open(keypair_file)?;
    let keypair_data: KeypairData = serde_json::from_reader(file)?;

    let client = reqwest::Client::new();

    // Detect server type
    let server_type = detect_identity_server_type(&client, identity_server_url).await?;

    match server_type {
        IdentityServerType::GitHub => {
            get_github_identity(
                &client,
                identity_server_url,
                username,
                keypair_data.public_key,
                output_file,
            )
            .await
        }
        IdentityServerType::Standard => {
            get_standard_identity(
                &client,
                identity_server_url,
                username,
                &keypair_data,
                output_file,
            )
            .await
        }
    }
}
