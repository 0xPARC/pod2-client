use std::fs::File;

use chrono::Utc;
use hex::FromHex;
use num_bigint::BigUint;
use pod_utils::{ValueExt, prover_setup::PodNetProverSetup};
use pod2::{
    backends::plonky2::{primitives::ec::schnorr::SecretKey, signedpod::Signer},
    frontend::{SignedPod, SignedPodBuilder},
    middleware::Hash,
};
use podnet_models::{
    UpvoteRequest,
    mainpod::upvote::{UpvoteProofParamsSolver, prove_upvote_verification_with_solver},
};
use reqwest::StatusCode;

use crate::utils::handle_error_response;

pub async fn upvote_document(
    keypair_file: &str,
    document_id: &str,
    server_url: &str,
    identity_pod_file: &str,
    use_mock: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "Upvoting document {document_id} on server {server_url} using main pod verification..."
    );

    // Parse document ID
    let doc_id: i64 = document_id.parse()?;

    // First, get the document to retrieve its content hash and post ID
    println!("Retrieving document {doc_id} to get content hash and post ID...");
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{server_url}/documents/{doc_id}"))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await?;
        handle_error_response(status, &error_text, "retrieve document");
        return Ok(());
    }

    let document: serde_json::Value = response.json().await?;
    let content_hash = document
        .pointer("/metadata/content_id")
        .and_then(|v| v.as_str())
        .map(Hash::from_hex)
        .ok_or("Document missing metadata.content_id field")??;

    let post_id = document
        .pointer("/metadata/post_id")
        .and_then(|v| v.as_i64())
        .ok_or("Document missing metadata.post_id field")?;

    println!("Document content hash: {content_hash}");
    println!("Document post ID: {post_id}");

    // Load and verify identity pod
    println!("Loading identity pod from: {identity_pod_file}");
    let identity_pod_json = std::fs::read_to_string(identity_pod_file)?;
    let identity_pod: SignedPod = serde_json::from_str(&identity_pod_json)?;

    // Verify the identity pod
    identity_pod.verify()?;
    println!("✓ Identity pod verification successful");

    // Load keypair from file
    let file = File::open(keypair_file)?;
    let keypair_data: serde_json::Value = serde_json::from_reader(file)?;

    let sk_hex = keypair_data["secret_key"]
        .as_str()
        .ok_or("Invalid keypair file: missing secret_key")?;
    let sk_bytes = hex::decode(sk_hex)?;
    let sk_bigint = BigUint::from_bytes_le(&sk_bytes);
    let secret_key = SecretKey(sk_bigint);

    println!("Using keypair: {}", keypair_data["name"]);
    println!("Public key: {}", keypair_data["public_key"]);

    // Create upvote pod with content hash, post ID, and request type
    let params = PodNetProverSetup::get_params();
    let mut upvote_builder = SignedPodBuilder::new(&params);

    upvote_builder.insert("request_type", "upvote");
    upvote_builder.insert("content_hash", content_hash);
    upvote_builder.insert("timestamp", Utc::now().timestamp());

    let upvote_pod = upvote_builder.sign(&Signer(secret_key))?;
    println!("UPVOTE POD: {upvote_pod}");
    println!("✓ Upvote pod signed successfully");

    // Verify the upvote pod
    upvote_pod.verify()?;
    println!("✓ Upvote pod verification successful");

    // Extract verification info manually
    let username = identity_pod
        .get("username")
        .and_then(|v| v.as_str())
        .ok_or("Identity pod missing username")?
        .to_string();

    println!("Username: {username}");

    // Create main pod that proves both identity and upvote verification using solver
    let params = UpvoteProofParamsSolver {
        identity_pod: &identity_pod,
        upvote_pod: &upvote_pod,
        use_mock_proofs: use_mock,
    };
    let main_pod = prove_upvote_verification_with_solver(params)
        .map_err(|e| format!("Failed to generate upvote verification MainPod: {e}"))?;

    println!("✓ Upvote main pod created and verified");

    // Create the upvote request using the proper struct
    let upvote_request = UpvoteRequest {
        username: username.clone(),
        upvote_main_pod: main_pod,
    };

    println!("Submitting upvote to server...");
    let response = client
        .post(format!("{server_url}/documents/{doc_id}/upvote"))
        .header("Content-Type", "application/json")
        .json(&upvote_request)
        .send()
        .await?;

    if response.status().is_success() {
        let result: serde_json::Value = response.json().await?;
        println!("✓ Successfully upvoted document using main pod verification!");

        if let Some(upvote_count) = result.get("upvote_count").and_then(|v| v.as_i64()) {
            println!("Document now has {upvote_count} upvotes");
        }

        println!(
            "Server response: {}",
            serde_json::to_string_pretty(&result)?
        );
    } else {
        let status = response.status();
        let error_text = response.text().await?;

        if status == StatusCode::CONFLICT {
            println!("You have already upvoted this document!");
        } else {
            handle_error_response(status, &error_text, "upvote document with main pod");
        }
    }

    Ok(())
}
