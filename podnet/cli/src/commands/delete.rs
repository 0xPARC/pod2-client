use std::fs::File;

use num_bigint::BigUint;
use pod_utils::{ValueExt, prover_setup::PodNetProverSetup};
use pod2::{
    backends::plonky2::{primitives::ec::schnorr::SecretKey, signedpod::Signer},
    frontend::{SignedPod, SignedPodBuilder},
};
use podnet_models::{
    DeleteRequest, Document,
    mainpod::delete::{DeleteProofParams, prove_delete},
    signed_pod,
};

use crate::utils::handle_error_response;

pub async fn delete_document(
    keypair_file: &str,
    document_id: &str,
    server_url: &str,
    identity_pod_file: &str,
    use_mock: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Deleting document {document_id} from server using main pod verification...");

    // Parse document ID
    let document_id_num: i64 = document_id
        .parse()
        .map_err(|_| format!("Invalid document ID: {document_id}"))?;

    // Load and verify identity pod
    println!("Loading identity pod from: {identity_pod_file}");
    let identity_pod_json = std::fs::read_to_string(identity_pod_file)?;
    let identity_pod: SignedPod = serde_json::from_str(&identity_pod_json)?;

    // Verify the identity pod
    identity_pod.verify()?;
    println!("✓ Identity pod verification successful");

    // Extract username from identity pod
    let username = identity_pod
        .get("username")
        .and_then(|v| v.as_str())
        .ok_or("Identity pod missing username")?
        .to_string();

    println!("Username: {username}");

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

    // Fetch the document from server to get the actual document pod and timestamp pod
    println!("Fetching document {document_id_num} from server...");
    let client = reqwest::Client::new();
    let document_response = client
        .get(format!("{server_url}/documents/{document_id_num}"))
        .send()
        .await?;

    if !document_response.status().is_success() {
        let status = document_response.status();
        let error_text = document_response.text().await?;
        handle_error_response(status, &error_text, "fetch document");
        return Ok(());
    }

    let document: Document = document_response.json().await?;
    println!("✓ Document fetched successfully");

    // Extract only the timestamp pod from the server response
    // We'll create our own document pod for the delete request
    let timestamp_pod = document.metadata.timestamp_pod.get()?;

    println!("✓ Timestamp pod extracted from server");

    // Verify the timestamp pod
    timestamp_pod.verify()?;
    println!("✓ Timestamp pod verification successful");

    // Extract the original data from the publish MainPod to use in delete pod
    let publish_main_pod = document.metadata.pod.get()?;

    // The publish MainPod contains the verified data structure - we need to extract it
    // The data is in the public statements of the MainPod
    let publish_verified_statement = &publish_main_pod.public_statements[1]; // publish_verified statement
    let original_data = match publish_verified_statement {
        pod2::middleware::Statement::Custom(_, args) => &args[1], // Second argument is the data
        _ => return Err("Invalid MainPod structure - expected publish_verified statement".into()),
    };

    println!("✓ Original document data extracted from publish MainPod");

    // Create document pod for deletion request (signed by user) using the same data
    let params = PodNetProverSetup::get_params();
    let delete_document_pod = signed_pod!(&params, secret_key, {
        "request_type" => "delete",
        "data" => original_data.clone(),
        "timestamp_pod" => timestamp_pod.id(),
    });

    // Verify the delete document pod
    delete_document_pod.verify()?;
    println!("✓ Delete document pod created and verified");

    // Create main pod that proves both identity and document verification
    let delete_params = DeleteProofParams {
        identity_pod: &identity_pod,
        document_pod: &delete_document_pod,
        timestamp_pod,
        use_mock_proofs: use_mock,
    };
    let main_pod = prove_delete(delete_params)
        .map_err(|e| format!("Failed to generate delete verification MainPod: {e}"))?;

    println!("✓ Main pod created and verified");

    // Create the delete request
    let delete_request = DeleteRequest {
        document_id: document_id_num,
        username: username.clone(),
        main_pod,
    };

    println!("Sending delete request");
    let response = client
        .delete(format!("{server_url}/documents/{document_id_num}"))
        .header("Content-Type", "application/json")
        .json(&delete_request)
        .send()
        .await?;

    if response.status().is_success() {
        let result: serde_json::Value = response.json().await?;
        println!("✓ Successfully deleted document from server using main pod verification!");
        println!(
            "Server response: {}",
            serde_json::to_string_pretty(&result)?
        );
    } else {
        let status = response.status();
        let error_text = response.text().await?;
        handle_error_response(status, &error_text, "delete document with main pod");
    }

    Ok(())
}
