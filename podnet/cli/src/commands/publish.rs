use std::{
    collections::{HashMap, HashSet},
    fs::File,
};

use num_bigint::BigUint;
use pod_utils::{ValueExt, prover_setup::PodNetProverSetup};
use pod2::{
    backends::plonky2::{primitives::ec::schnorr::SecretKey, signedpod::Signer},
    frontend::{SignedPod, SignedPodBuilder},
    middleware::{
        KEY_SIGNER, Key, Value,
        containers::{Dictionary, Set},
        hash_values,
    },
};
use podnet_models::{
    DocumentContent, DocumentFile, PublishRequest,
    mainpod::publish::{
        PublishProofParams, prove_publish_verification, prove_publish_verification_with_solver,
        verify_publish_verification_with_solver,
    },
    signed_pod,
};

use crate::{
    conversion::{DocumentFormat, convert_to_markdown},
    utils::handle_error_response,
};

pub async fn publish_content(
    keypair_file: &str,
    title: &str,
    message: Option<&String>,
    file_path: Option<&String>,
    url: Option<&String>,
    format_override: Option<&String>,
    server_url: &str,
    post_id: Option<&String>,
    identity_pod_file: &str,
    use_mock: bool,
    tags: Option<&String>,
    authors: Option<&String>,
    reply_to: Option<&String>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Publishing content to server using main pod verification...");

    // Step 1: Build DocumentContent from provided inputs
    let mut document_content = DocumentContent {
        message: None,
        file: None,
        url: None,
    };

    // Process message
    if let Some(msg) = message {
        // Handle format conversion if needed
        let processed_message = if let Some(format_str) = format_override {
            let detected_format = DocumentFormat::from_str(format_str)
                .ok_or_else(|| format!("Invalid format: {format_str}"))?;
            if detected_format != DocumentFormat::Markdown {
                let converted = convert_to_markdown(msg, &detected_format)?;
                println!("✓ Message converted from {detected_format:?} to Markdown");
                converted
            } else {
                msg.clone()
            }
        } else {
            msg.clone()
        };
        document_content.message = Some(processed_message);
        println!("Message added to document");
    }

    // Process file
    if let Some(file_path_str) = file_path {
        println!("Reading file: {file_path_str}");
        let file_content = std::fs::read(file_path_str)?;
        let file_name = std::path::Path::new(file_path_str)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Detect MIME type based on file extension
        let mime_type = match std::path::Path::new(file_path_str)
            .extension()
            .and_then(|ext| ext.to_str())
        {
            Some("txt") => "text/plain",
            Some("md") => "text/markdown",
            Some("jpg") | Some("jpeg") => "image/jpeg",
            Some("png") => "image/png",
            Some("pdf") => "application/pdf",
            Some("json") => "application/json",
            _ => "application/octet-stream",
        }
        .to_string();

        document_content.file = Some(DocumentFile {
            name: file_name,
            content: file_content,
            mime_type,
        });
        println!("File added to document");
    }

    // Process URL
    if let Some(url_str) = url {
        document_content.url = Some(url_str.clone());
        println!("URL added to document: {url_str}");
    }

    // Validate that at least one content type is provided
    document_content
        .validate()
        .map_err(|e| format!("Content validation failed: {e}"))?;

    // Step 3: Process tags
    let document_tags: HashSet<String> = if let Some(tags_str) = tags {
        tags_str
            .split(',')
            .map(|tag| tag.trim().to_string())
            .filter(|tag| !tag.is_empty())
            .collect()
    } else {
        HashSet::new()
    };

    if !document_tags.is_empty() {
        println!(
            "Tags: {}",
            document_tags
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    // Load and verify identity pod
    println!("Loading identity pod from: {identity_pod_file}");
    let identity_pod_json = std::fs::read_to_string(identity_pod_file)?;
    let identity_pod: SignedPod = serde_json::from_str(&identity_pod_json)?;

    // Verify the identity pod
    identity_pod.verify()?;
    println!("✓ Identity pod verification successful");

    // Extract username from identity pod for authors default
    let username = identity_pod
        .get("username")
        .and_then(|v| v.as_str())
        .ok_or("Identity pod missing username")?
        .to_string();

    println!("Username: {username}");

    // Compute content hash from the entire DocumentContent structure
    let content_json = serde_json::to_string(&document_content)?;
    let content_hash = hash_values(&[Value::from(content_json)]);
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
    println!("Content hash: {content_hash}");
    println!("Tags: {document_tags:?}");

    // Process authors (default to uploader if not provided or empty)
    let document_authors: HashSet<String> = if let Some(authors_str) = authors {
        let parsed_authors: HashSet<String> = authors_str
            .split(',')
            .map(|author| author.trim().to_string())
            .filter(|author| !author.is_empty())
            .collect();

        if parsed_authors.is_empty() {
            // If authors string was provided but empty, default to uploader
            let mut default_authors = HashSet::new();
            default_authors.insert(username.clone());
            default_authors
        } else {
            parsed_authors
        }
    } else {
        // If no authors provided, default to uploader
        let mut default_authors = HashSet::new();
        default_authors.insert(username.clone());
        default_authors
    };

    let post_id_num = post_id.map(|id| id.parse::<i64>()).transpose()?;
    let reply_to_num = reply_to.map(|id| id.parse::<i64>()).transpose()?;

    let tag_set = Set::new(
        5, // TODO: put this configuration somewhere global
        document_tags
            .iter()
            .map(|v| Value::from(v.clone()))
            .collect(),
    )?;

    let authors_set = Set::new(
        5,
        document_authors
            .iter()
            .map(|author| Value::from(author.as_str()))
            .collect(),
    )?;

    let data_dict = Dictionary::new(
        6,
        HashMap::from([
            (Key::from("authors"), Value::from(authors_set)),
            (Key::from("content_hash"), Value::from(content_hash)),
            (Key::from("tags"), Value::from(tag_set)),
            (Key::from("post_id"), Value::from(post_id_num.unwrap_or(-1))),
            (
                Key::from("reply_to"),
                Value::from(reply_to_num.unwrap_or(-1)),
            ),
        ]),
    )?;

    // Create document pod
    let params = PodNetProverSetup::get_params();
    let document_pod = signed_pod!(&params, secret_key, {
        "request_type" => "publish",
        "data" => data_dict.clone(),
    });
    println!("✓ Document pods signed successfully");

    // Verify the document pods
    document_pod.verify()?;
    println!("✓ Document pods verification successful");

    // Create main pod that proves both identity and document verification
    let params = PublishProofParams {
        identity_pod: &identity_pod,
        document_pod: &document_pod,
        use_mock_proofs: use_mock,
    };
    let main_pod = prove_publish_verification_with_solver(params)
        .map_err(|e| format!("Failed to generate publish verification MainPod: {e}"))?;
    verify_publish_verification_with_solver(
        &main_pod,
        &username,
        &data_dict,
        identity_pod.get(KEY_SIGNER).unwrap(),
    )
    .map_err(|e| format!("Failed to verify publish verification MainPod: {e}"))?;

    println!("✓ Main pod created and verified");

    // Process reply_to parameter
    let reply_to_id: Option<i64> = if let Some(reply_to_str) = reply_to {
        match reply_to_str.parse::<i64>() {
            Ok(id) => {
                println!("Replying to document ID: {id}");
                Some(id)
            }
            Err(_) => {
                return Err(format!("Invalid reply_to document ID: {reply_to_str}").into());
            }
        }
    } else {
        None
    };

    println!("Creating publish request");
    // Create the publish request using the proper struct
    let publish_request = PublishRequest {
        title: title.to_string(),
        content: document_content,
        tags: document_tags,
        authors: document_authors,
        reply_to: reply_to_id,
        post_id: post_id_num,
        username: username.clone(),
        main_pod,
    };
    println!("Main pod is: {}", &publish_request.main_pod);

    println!("Sending publish request");
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{server_url}/publish"))
        .header("Content-Type", "application/json")
        .json(&publish_request)
        .send()
        .await?;
    println!("Done! mainpod");

    if response.status().is_success() {
        let result: serde_json::Value = response.json().await?;
        println!("✓ Successfully published to server using main pod verification!");
        println!(
            "Server response: {}",
            serde_json::to_string_pretty(&result)?
        );
    } else {
        let status = response.status();
        let error_text = response.text().await?;
        handle_error_response(status, &error_text, "publish with main pod");
    }

    Ok(())
}
