// Helper functions for extracting and formatting data

pub fn extract_document_metadata(document: &serde_json::Value) -> (String, String, i64, i64) {
    let content_id = document
        .get("content_id")
        .and_then(|v| v.as_str())
        .unwrap_or("N/A")
        .to_string();
    let created_at = document
        .get("created_at")
        .and_then(|v| v.as_str())
        .unwrap_or("N/A")
        .to_string();
    let post_id = document
        .get("post_id")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let revision = document
        .get("revision")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    (content_id, created_at, post_id, revision)
}

pub fn handle_error_response(status: reqwest::StatusCode, error_text: &str, operation: &str) {
    println!("Failed to {operation}. Status: {status}");
    println!("Error: {error_text}");
}

pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[0..max_len - 3])
    } else {
        s.to_string()
    }
}

pub fn truncate_pod_json(pod_json: &str) -> String {
    truncate_string(pod_json, 50)
}

pub async fn get_server_public_key(server_url: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let response = client.get(server_url).send().await?;

    if response.status().is_success() {
        let server_info: serde_json::Value = response.json().await?;
        let public_key = server_info
            .get("public_key")
            .and_then(|v| v.as_str())
            .ok_or("Server response missing public_key")?;
        Ok(public_key.to_string())
    } else {
        Err("Failed to get server info".into())
    }
}
