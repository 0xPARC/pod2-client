use crate::utils::{handle_error_response, extract_document_metadata, truncate_pod_json};

pub async fn get_post_by_id(post_id: &str, server_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let response = client
        .get(format!("{server_url}/posts/{post_id}"))
        .send()
        .await?;

    if response.status().is_success() {
        let post: serde_json::Value = response.json().await?;

        println!("Post ID: {post_id}");
        println!(
            "Created: {}",
            post.get("created_at")
                .and_then(|v| v.as_str())
                .unwrap_or("N/A")
        );
        println!(
            "Last Edited: {}",
            post.get("last_edited_at")
                .and_then(|v| v.as_str())
                .unwrap_or("N/A")
        );

        if let Some(documents) = post.get("documents").and_then(|d| d.as_array()) {
            println!("\nDocuments ({} revisions):", documents.len());
            for document in documents {
                let (content_id, created_at, _, revision) = extract_document_metadata(document);
                println!("  Revision {revision}: {content_id} ({created_at})");

                if let Some(content) = document.get("content").and_then(|v| v.as_str()) {
                    println!(
                        "    Content: {}",
                        if content.len() > 100 {
                            format!("{}...", &content[0..100])
                        } else {
                            content.to_string()
                        }
                    );
                }
            }
        } else {
            println!("No documents found for this post.");
        }
    } else {
        let status = response.status();
        let error_text = response.text().await?;
        handle_error_response(status, &error_text, "retrieve post");
    }

    Ok(())
}

pub async fn list_posts(server_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let response = client.get(format!("{server_url}/posts")).send().await?;

    if response.status().is_success() {
        let posts: serde_json::Value = response.json().await?;

        if let Some(posts_array) = posts.as_array() {
            if posts_array.is_empty() {
                println!("No posts found.");
                return Ok(());
            }

            println!("Available posts:");
            println!("{:<6} {:<20} {:<20} {:<8} {:<52} {:<52}", "ID", "Created", "Last Edited", "Docs", "Latest Main Pod", "Latest Timestamp Pod");
            println!("{}", "-".repeat(160));

            for post in posts_array {
                print_post_row(post);
            }
        } else {
            println!("Invalid response format from server.");
        }
    } else {
        let status = response.status();
        let error_text = response.text().await?;
        handle_error_response(status, &error_text, "list posts");
    }

    println!("\nUse 'get-post --post-id <ID>' to retrieve post details.");

    Ok(())
}

fn print_post_row(post: &serde_json::Value) {
    let id = post.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
    let created_at = post
        .get("created_at")
        .and_then(|v| v.as_str())
        .unwrap_or("N/A");
    let last_edited_at = post
        .get("last_edited_at")
        .and_then(|v| v.as_str())
        .unwrap_or("N/A");
    let documents = post
        .get("documents")
        .and_then(|d| d.as_array());
    let document_count = documents.map(|docs| docs.len()).unwrap_or(0);

    // Get latest document's pods (highest revision number)
    let (latest_main_pod, latest_timestamp_pod) = if let Some(docs) = documents {
        if let Some(latest_doc) = docs.iter().max_by_key(|doc| {
            doc.get("revision").and_then(|v| v.as_i64()).unwrap_or(0)
        }) {
            let main_pod = latest_doc
                .get("pod")
                .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "Invalid JSON".to_string()))
                .unwrap_or_else(|| "N/A".to_string());
            let timestamp_pod = latest_doc
                .get("timestamp_pod")
                .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "Invalid JSON".to_string()))
                .unwrap_or_else(|| "N/A".to_string());
            (main_pod, timestamp_pod)
        } else {
            ("N/A".to_string(), "N/A".to_string())
        }
    } else {
        ("N/A".to_string(), "N/A".to_string())
    };

    println!(
        "{:<6} {:<20} {:<20} {:<8} {:<52} {:<52}",
        id, created_at, last_edited_at, document_count,
        truncate_pod_json(&latest_main_pod), truncate_pod_json(&latest_timestamp_pod)
    );
}