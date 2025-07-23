use crate::utils::{handle_error_response, truncate_pod_json};

//pub async fn get_document_by_id(
//    document_id: &str,
//    server_url: &str,
//) -> Result<(), Box<dyn std::error::Error>> {
//    let client = reqwest::Client::new();
//    let response = client
//        .get(format!("{server_url}/documents/{document_id}"))
//        .send()
//        .await?;
//
//    if response.status().is_success() {
//        let document: serde_json::Value = response.json().await?;
//
//        if let Some(content) = document.get("content").and_then(|v| v.as_str()) {
//            let (content_id, created_at, post_id, revision) = extract_document_metadata(&document);
//            let upvote_count = document.pointer("/metadata/upvote_count").and_then(|v| v.as_i64()).unwrap_or(0);
//
//            // Verify upvote count pod if present
//            if let Some(upvote_count_pod_json) = document.pointer("/metadata/upvote_count_pod") {
//                match serde_json::from_value(upvote_count_pod_json.clone()) {
//                    Ok(upvote_count_pod) => {
//                        match verify_upvote_count(&upvote_count_pod, upvote_count, &content_id) {
//                            Ok(()) => {
//                                println!("✓ Upvote count verification successful (count: {})", upvote_count);
//                            }
//                            Err(e) => {
//                                println!("⚠️  Upvote count verification failed: {}", e);
//                            }
//                        }
//                    }
//                    Err(e) => {
//                        println!("⚠️  Failed to parse upvote count pod: {}", e);
//                    }
//                }
//            } else if upvote_count > 0 {
//                println!("⚠️  Warning: Document claims {} upvotes but no upvote count proof provided", upvote_count);
//            }
//
//            print_document_metadata(&content_id, &created_at, post_id, revision, upvote_count);
//            println!("Content:\n{content}");
//        } else {
//            println!("No content found for document ID: {document_id}");
//        }
//    } else {
//        let status = response.status();
//        let error_text = response.text().await?;
//        handle_error_response(status, &error_text, "retrieve document");
//    }
//
//    Ok(())
//}
//
//pub async fn render_document_by_id(
//    document_id: &str,
//    server_url: &str,
//) -> Result<(), Box<dyn std::error::Error>> {
//    let client = reqwest::Client::new();
//    let response = client
//        .get(format!("{server_url}/documents/{document_id}/render"))
//        .send()
//        .await?;
//
//    if response.status().is_success() {
//        let document: serde_json::Value = response.json().await?;
//
//        if let Some(content) = document.get("content").and_then(|v| v.as_str()) {
//            let (content_id, created_at, post_id, revision) = extract_document_metadata(&document);
//            let upvote_count = document.pointer("/metadata/upvote_count").and_then(|v| v.as_i64()).unwrap_or(0);
//
//            // Verify upvote count pod if present
//            if let Some(upvote_count_pod_json) = document.pointer("/metadata/upvote_count_pod") {
//                match serde_json::from_value(upvote_count_pod_json.clone()) {
//                    Ok(upvote_count_pod) => {
//                        match verify_upvote_count(&upvote_count_pod, upvote_count, &content_id) {
//                            Ok(()) => {
//                                println!("✓ Upvote count verification successful (count: {})", upvote_count);
//                            }
//                            Err(e) => {
//                                println!("⚠️  Upvote count verification failed: {}", e);
//                            }
//                        }
//                    }
//                    Err(e) => {
//                        println!("⚠️  Failed to parse upvote count pod: {}", e);
//                    }
//                }
//            } else if upvote_count > 0 {
//                println!("⚠️  Warning: Document claims {} upvotes but no upvote count proof provided", upvote_count);
//            }
//
//            print_document_metadata(&content_id, &created_at, post_id, revision, upvote_count);
//            println!("Rendered HTML:\n{content}");
//        } else {
//            println!("No content found for document ID: {document_id}");
//        }
//    } else {
//        let status = response.status();
//        let error_text = response.text().await?;
//        handle_error_response(status, &error_text, "render document");
//    }
//
//    Ok(())
//}

pub async fn list_documents(server_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let response = client.get(format!("{server_url}/documents")).send().await?;

    if response.status().is_success() {
        let documents: serde_json::Value = response.json().await?;

        if let Some(documents_array) = documents.as_array() {
            if documents_array.is_empty() {
                println!("No documents found.");
                return Ok(());
            }

            println!("Available documents:");
            println!(
                "{:<6} {:<12} {:<8} {:<8} {:<20} {:<30} {:<66} {:<52} {:<52}",
                "ID",
                "Post",
                "Rev",
                "Upvotes",
                "Created",
                "Tags",
                "Content Hash",
                "Main Pod",
                "Timestamp Pod"
            );
            println!("{}", "-".repeat(288));

            for document in documents_array {
                print_document_row(document);
            }
        } else {
            println!("Invalid response format from server.");
        }
    } else {
        let status = response.status();
        let error_text = response.text().await?;
        handle_error_response(status, &error_text, "list documents");
    }

    println!(
        "\nUse 'get-document --document-id <ID>' to retrieve document or 'view --post-id <POST_ID>' to open latest document in browser."
    );

    Ok(())
}

fn print_document_row(document: &serde_json::Value) {
    let id = document.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let content_id = document
        .get("content_id")
        .and_then(|v| v.as_str())
        .unwrap_or("N/A");
    let post_id = document
        .get("post_id")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let revision = document
        .get("revision")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let upvote_count = document
        .get("upvote_count")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let created_at = document
        .get("created_at")
        .and_then(|v| v.as_str())
        .unwrap_or("N/A");

    // Extract and format tags
    let tags = document
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|tags_array| {
            let tag_strings: Vec<String> = tags_array
                .iter()
                .filter_map(|tag| tag.as_str().map(|s| s.to_string()))
                .collect();
            if tag_strings.is_empty() {
                "".to_string()
            } else {
                tag_strings.join(", ")
            }
        })
        .unwrap_or_default();

    // Truncate tags for display (max 28 chars to fit in column)
    let display_tags = if tags.len() > 28 {
        format!("{}...", &tags[0..25])
    } else {
        tags
    };

    // Truncate content_id for display
    let display_content_id = if content_id.len() > 64 {
        format!("{}...", &content_id[0..61])
    } else {
        content_id.to_string()
    };

    // Get main pod and timestamp pod (server sends as JSON objects)
    let main_pod = document
        .get("pod")
        .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "Invalid JSON".to_string()))
        .unwrap_or("N/A".to_string());
    let timestamp_pod = document
        .get("timestamp_pod")
        .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "Invalid JSON".to_string()))
        .unwrap_or("N/A".to_string());

    println!(
        "{:<6} {:<12} {:<8} {:<8} {:<20} {:<30} {:<66} {:<52} {:<52}",
        id,
        post_id,
        revision,
        upvote_count,
        created_at,
        display_tags,
        display_content_id,
        truncate_pod_json(&main_pod),
        truncate_pod_json(&timestamp_pod)
    );
}
