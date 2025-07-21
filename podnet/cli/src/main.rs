mod cli;
mod commands;
mod config;
mod conversion;
mod utils;

use clap::{Arg, Command};
use cli::*;
use commands::{documents, identity, keygen, posts, publish, upvote};
use hex::ToHex;
use podnet_models::DocumentContent;
use pulldown_cmark::{Event, Options, Parser, html};
use utils::*;

fn render_to_html(
    id: &str,
    content_id: &str,
    timestamp: &str,
    uploader: &str,
    tags: &std::collections::HashSet<String>,
    authors: &std::collections::HashSet<String>,
    html_content: &str,
    revision_links: &str,
) -> String {
    // Format tags for display
    let tags_display = if tags.is_empty() {
        "None".to_string()
    } else {
        let tag_list: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();
        tag_list.join(", ")
    };

    let authors_display = if authors.is_empty() {
        "None".to_string()
    } else {
        let authors_list: Vec<&str> = authors.iter().map(|s| s.as_str()).collect();
        authors_list.join(", ")
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>PodNet Content - Post {id}</title>
    <script>
MathJax = {{
  tex: {{
    inlineMath: [['$', '$'], ['\\(', '\\)']],
    displayMath: [['$$', '$$'], ['\\[', '\\]']],
    processEscapes: true,
    processEnvironments: true,
    packages: {{'[+]': ['textcomp', 'textmacros']}},
    macros: {{
      textbf: ['\\mathbf{{#1}}', 1],
      texttt: ['\\mathtt{{#1}}', 1]
    }}
  }},
  options: {{
    ignoreHtmlClass: 'tex2jax_ignore',
    processHtmlClass: 'tex2jax_process'
  }}
}};
</script>
<script type="text/javascript" id="MathJax-script" async
  src="https://cdn.jsdelivr.net/npm/mathjax@3/es5/tex-mml-chtml.js">
</script>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Roboto', sans-serif;
            line-height: 1.6;
            margin: 0;
            padding: 0;
            color: #333;
            background-color: #fff;
            display: flex;
            min-height: 100vh;
        }}
        .sidebar {{
            width: 250px;
            background-color: #f8f9fa;
            border-right: 2px solid #eee;
            padding: 20px;
            position: fixed;
            height: 100vh;
            overflow-y: auto;
            box-sizing: border-box;
        }}
        .main-content {{
            flex: 1;
            margin-left: 250px;
            padding: 20px;
            max-width: 800px;
        }}
        .header {{
            border-bottom: 2px solid #eee;
            padding-bottom: 20px;
            margin-bottom: 30px;
        }}
        .metadata {{
            background-color: #f8f9fa;
            padding: 15px;
            border-radius: 5px;
            margin-bottom: 20px;
            font-size: 0.9em;
            color: #666;
        }}
        .metadata strong {{
            color: #333;
        }}
        .sidebar h3 {{
            margin-top: 0;
            color: #1976d2;
            border-bottom: 2px solid #2196f3;
            padding-bottom: 10px;
        }}
        .revision-item {{
            margin-bottom: 10px;
            padding: 10px;
            border-radius: 5px;
            cursor: pointer;
            transition: background-color 0.2s;
        }}
        .revision-item:hover {{
            background-color: #e3f2fd;
        }}
        .revision-item.current {{
            background-color: #e8f5e8;
            border-left: 4px solid #28a745;
        }}
        .revision-link {{
            color: #1976d2;
            text-decoration: none;
            display: block;
            font-weight: 500;
        }}
        .revision-link:hover {{
            text-decoration: underline;
        }}
        .revision-date {{
            font-size: 0.8em;
            color: #666;
            margin-top: 5px;
        }}
        .loading {{
            text-align: center;
            padding: 20px;
            color: #666;
        }}
        pre {{
            background-color: #f8f9fa;
            padding: 15px;
            border-radius: 5px;
            overflow-x: auto;
            border-left: 4px solid #007bff;
        }}
        code {{
            background-color: #f8f9fa;
            padding: 2px 6px;
            border-radius: 3px;
            font-family: 'Monaco', 'Consolas', monospace;
        }}
        pre code {{
            background-color: transparent;
            padding: 0;
        }}
        h1, h2, h3, h4, h5, h6 {{
            margin-top: 30px;
            margin-bottom: 15px;
        }}
        h1 {{
            color: #2c3e50;
            border-bottom: 2px solid #eee;
            padding-bottom: 10px;
        }}
        h2 {{
            color: #34495e;
        }}
        ul, ol {{
            padding-left: 30px;
        }}
        li {{
            margin-bottom: 5px;
        }}
        a {{
            color: #007bff;
            text-decoration: none;
        }}
        a:hover {{
            text-decoration: underline;
        }}
    </style>
    <script>
        function showRevision(docId) {{
            // Hide all document content divs
            document.querySelectorAll('.document-content').forEach(div => {{
                div.style.display = 'none';
            }});
            
            // Show the selected document
            const selectedDoc = document.getElementById(`document-${{docId}}`);
            if (selectedDoc) {{
                selectedDoc.style.display = 'block';
                
                // Update metadata with the selected document's info
                const metadata = document.querySelector('.metadata');
                const contentId = selectedDoc.getAttribute('data-content-id') || 'N/A';
                const timestamp = selectedDoc.getAttribute('data-created') || 'N/A';
                const uploader = selectedDoc.getAttribute('data-uploader') || 'Unknown';
                const revision = selectedDoc.getAttribute('data-revision') || 'N/A';
                
                metadata.innerHTML = `
                    <div><strong>Post ID:</strong> {id}</div>
                    <div><strong>Uploader:</strong> ${{uploader}}</div>
                    <div><strong>Content Hash:</strong> <code>${{contentId}}</code></div>
                    <div><strong>Timestamp:</strong> ${{timestamp}}</div>
                    <div><strong>Revision:</strong> ${{revision}}</div>
                `;
                
                // Update current revision highlight in sidebar
                document.querySelectorAll('.revision-item').forEach(item => {{
                    item.classList.remove('current');
                }});
                document.querySelector(`[data-doc-id="${{docId}}"]`).classList.add('current');
            }}
        }}
    </script>
</head>
<body>
    <div class="sidebar">
        <h3>Revisions</h3>
        {revision_links}
    </div>
    <div class="main-content">
        <div class="header">
            <h1>PodNet Content</h1>
            <div class="metadata">
                <div><strong>Post ID:</strong> {id}</div>
                <div><strong>Uploader:</strong> {uploader}</div>
                <div><strong>Authors:</strong> {authors_display}</div>
                <div><strong>Content Hash:</strong> <code>{content_id}</code></div>
                <div><strong>Timestamp:</strong> {timestamp}</div>
                <div><strong>Tags:</strong> {tags_display}</div>
            </div>
        </div>
        <div id="main-content" class="content">
            {html_content}
        </div>
    </div>
</body>
</html>"#
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("parcnet-cli")
        .version("1.0")
        .about("CLI tool for parcnet pod2 operations")
        .subcommand(
            Command::new("keygen")
                .about("Generate a Schnorr keypair for pod2 signing")
                .args([
                    Arg::new("output")
                        .help("Output file for the keypair")
                        .short('o')
                        .long("output")
                        .default_value("keypair.json"),
                ]),
        )
        .subcommand(
            Command::new("publish")
                .about("Sign content and submit to server using main pod verification (creates new post or adds revision)")
                .args([
                    keypair_arg(),
                    optional_post_id_arg(),
                    Arg::new("title")
                        .help("Document title")
                        .long("title")
                        .value_name("TITLE")
                        .required(true),
                    Arg::new("identity_pod")
                        .help("Path to identity pod file")
                        .short('i')
                        .long("identity-pod")
                        .required(true),
                    Arg::new("mock")
                        .help("Use mock prover for faster development")
                        .long("mock")
                        .action(clap::ArgAction::SetTrue),
                    Arg::new("tags")
                        .help("Comma-separated list of tags for document organization")
                        .short('t')
                        .long("tags")
                        .value_name("TAG1,TAG2,TAG3"),
                    Arg::new("authors")
                        .help("Optional comma-separated list of authors for document attribution (defaults to uploader)")
                        .short('a')
                        .long("authors")
                        .value_name("AUTHOR1,AUTHOR2,AUTHOR3"),
                    Arg::new("reply_to")
                        .help("Optional document ID to reply to")
                        .short('r')
                        .long("reply-to")
                        .value_name("DOCUMENT_ID"),
                    Arg::new("message")
                        .help("Text message content")
                        .short('m')
                        .long("message")
                        .value_name("MESSAGE"),
                    Arg::new("file")
                        .help("File to attach")
                        .short('f')
                        .long("file")
                        .value_name("FILE_PATH"),
                    Arg::new("url")
                        .help("URL to reference")
                        .short('u')
                        .long("url")
                        .value_name("URL"),
                    Arg::new("format")
                        .help("Format override for message content")
                        .long("format")
                        .value_name("FORMAT"),
                ]),
        )
        .subcommand(
            Command::new("get-post")
                .about("Retrieve post with all its documents")
                .args([post_id_arg()]),
        )
        .subcommand(
            Command::new("get-document")
                .about("Retrieve specific document by ID")
                .args([document_id_arg()]),
        )
        .subcommand(
            Command::new("render")
                .about("Retrieve and render document as HTML by ID")
                .args([document_id_arg()]),
        )
        .subcommand(
            Command::new("view")
                .about("Retrieve latest document from post, render as HTML, and open in browser")
                .args([post_id_arg()]),
        )
        .subcommand(
            Command::new("list-posts")
                .about("List all posts")
        )
        .subcommand(
            Command::new("list-documents")
                .about("List all documents metadata")
        )
        .subcommand(
            Command::new("get-identity")
                .about("Get identity pod from identity server")
                .args([
                    keypair_arg(),
                    Arg::new("username")
                        .help("Username to register")
                        .short('u')
                        .long("username")
                        .required(true),
                    Arg::new("output")
                        .help("Output file for identity pod")
                        .short('o')
                        .long("output")
                        .default_value("identity.pod"),
                ]),
        )
        .subcommand(
            Command::new("upvote")
                .about("Upvote a document with cryptographic verification using main pod")
                .args([
                    keypair_arg(),
                    document_id_arg(),
                    identity_pod_arg(),
                    mock_arg(),
                ]),
        )
        .get_matches();

    match matches.subcommand() {
        Some(("keygen", sub_matches)) => {
            let output_file = sub_matches.get_one::<String>("output").unwrap();
            keygen::generate_keypair(output_file)?;
        }
        Some(("publish", sub_matches)) => {
            let keypair_file = sub_matches.get_one::<String>("keypair").unwrap();
            let title = sub_matches.get_one::<String>("title").unwrap();
            let message = sub_matches.get_one::<String>("message");
            let file_path = sub_matches.get_one::<String>("file");
            let url = sub_matches.get_one::<String>("url");
            let format_override = sub_matches.get_one::<String>("format");
            let server = config::CliConfig::load().server_url;
            let post_id = sub_matches.get_one::<String>("post_id");
            let identity_pod_file = sub_matches.get_one::<String>("identity_pod").unwrap();
            let use_mock = sub_matches.get_flag("mock");
            let tags = sub_matches.get_one::<String>("tags");
            let authors = sub_matches.get_one::<String>("authors");
            let reply_to = sub_matches.get_one::<String>("reply_to");

            // Validate that at least one content type is provided
            if message.is_none() && file_path.is_none() && url.is_none() {
                return Err("At least one of --message, --file, or --url must be provided".into());
            }

            publish::publish_content(
                keypair_file,
                title,
                message,
                file_path,
                url,
                format_override,
                &server,
                post_id,
                identity_pod_file,
                use_mock,
                tags,
                authors,
                reply_to,
            )
            .await?;
        }
        Some(("get-post", sub_matches)) => {
            let post_id = sub_matches.get_one::<String>("post_id").unwrap();
            let server = config::CliConfig::load().server_url;
            posts::get_post_by_id(post_id, &server).await?;
        }
        Some(("view", sub_matches)) => {
            let post_id = sub_matches.get_one::<String>("post_id").unwrap();
            let server = config::CliConfig::load().server_url;
            view_post_in_browser(post_id, &server).await?;
        }
        Some(("list-posts", sub_matches)) => {
            let server = config::CliConfig::load().server_url;
            posts::list_posts(&server).await?;
        }
        Some(("list-documents", sub_matches)) => {
            let server = config::CliConfig::load().server_url;
            documents::list_documents(&server).await?;
        }
        Some(("get-identity", sub_matches)) => {
            let keypair_file = sub_matches.get_one::<String>("keypair").unwrap();
            let identity_server = config::CliConfig::load().identity_server_url;
            let username = sub_matches.get_one::<String>("username").unwrap();
            let output_file = sub_matches.get_one::<String>("output").unwrap();
            identity::get_identity(keypair_file, &identity_server, username, output_file).await?;
        }
        Some(("upvote", sub_matches)) => {
            let keypair_file = sub_matches.get_one::<String>("keypair").unwrap();
            let server = config::CliConfig::load().server_url;
            let document_id = sub_matches.get_one::<String>("document_id").unwrap();
            let identity_pod = sub_matches.get_one::<String>("identity_pod").unwrap();
            let use_mock = sub_matches.get_flag("mock");
            upvote::upvote_document(keypair_file, document_id, &server, identity_pod, use_mock)
                .await?;
        }
        _ => {
            println!("No valid subcommand provided. Use --help for usage information.");
        }
    }

    Ok(())
}

async fn view_post_in_browser(
    post_id: &str,
    server_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();

    // Get server public key for signature verification
    let server_public_key = get_server_public_key(server_url).await?;
    println!("Server public key: {server_public_key}");

    // First get the post with all its documents
    let response = client
        .get(format!("{server_url}/posts/{post_id}"))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await?;
        handle_error_response(status, &error_text, "retrieve post");
        return Ok(());
    }

    let post: serde_json::Value = response.json().await?;

    // Find all documents (documents are ordered by revision DESC)
    let documents = post
        .get("documents")
        .and_then(|d| d.as_array())
        .ok_or("Post missing documents field")?;

    if documents.is_empty() {
        println!("No documents found for post ID: {post_id}");
        return Ok(());
    }

    // Fetch rendered content for ALL documents
    println!("Fetching all document revisions...");
    let mut document_data = Vec::new();

    for doc in documents.iter() {
        let doc_id = doc.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
        let doc_revision = doc.get("revision").and_then(|v| v.as_i64()).unwrap_or(0);

        println!("Fetching revision {doc_revision}...");

        // Get document content (no longer using /render endpoint)
        let document_response = client
            .get(format!("{server_url}/documents/{doc_id}"))
            .send()
            .await?;

        if !document_response.status().is_success() {
            let status = document_response.status();
            let error_text = document_response.text().await?;
            handle_error_response(status, &error_text, &format!("retrieve document {doc_id}"));
            continue; // Skip this document but continue with others
        }

        let document_json: serde_json::Value = document_response.json().await?;

        // Parse the document directly using the shared types
        let document: podnet_models::Document = serde_json::from_value(document_json.clone())
            .map_err(|e| format!("Failed to parse document: {e}"))?;

        // Render the raw DocumentContent to HTML on the client side
        let html_content = render_document_content_to_html(&document.content);

        let content_id = document.metadata.content_id;
        let created_at = document
            .metadata
            .created_at
            .as_deref()
            .unwrap_or("Unknown")
            .to_string();
        let revision = document.metadata.revision;
        let uploader_username = document.metadata.uploader_id.clone();
        let upvote_count = document.metadata.upvote_count;
        let tags = document.metadata.tags.clone();
        let authors = document.metadata.authors.clone();

        // Verify all cryptographic proofs using the new Document.verify() method
        println!("Verifying signatures for revision {revision}...");
        document.verify(&server_public_key)?;
        println!("Main pod: {}", document.metadata.pod.json());
        println!("Timestamp pod: {}", document.metadata.timestamp_pod.json());

        // Fetch replies for this document
        println!("Fetching replies for document {doc_id}...");
        let replies_response = client
            .get(format!("{server_url}/documents/{doc_id}/replies"))
            .send()
            .await?;

        let mut replies_html = String::new();
        if replies_response.status().is_success() {
            let replies: serde_json::Value = replies_response.json().await?;
            if let Some(replies_array) = replies.as_array() {
                if !replies_array.is_empty() {
                    replies_html.push_str("<div class=\"replies-section\" style=\"margin-top: 30px; border-top: 2px solid #eee; padding-top: 20px;\">");
                    replies_html.push_str(&format!(
                        "<h3>Replies ({} replies)</h3>",
                        replies_array.len()
                    ));

                    for reply in replies_array {
                        let reply_id = reply.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                        let reply_uploader = reply
                            .get("uploader_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown");
                        let reply_created = reply
                            .get("created_at")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown");
                        let reply_upvotes = reply
                            .get("upvote_count")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);

                        // Fetch the reply content
                        let reply_response = client
                            .get(format!("{server_url}/documents/{reply_id}/render"))
                            .send()
                            .await;

                        let reply_content = if let Ok(reply_resp) = reply_response {
                            if reply_resp.status().is_success() {
                                if let Ok(reply_doc_value) =
                                    reply_resp.json::<serde_json::Value>().await
                                {
                                    // Try to parse as Document and render the content
                                    if let Ok(reply_doc) =
                                        serde_json::from_value::<podnet_models::Document>(
                                            reply_doc_value,
                                        )
                                    {
                                        render_document_content_to_html(&reply_doc.content)
                                    } else {
                                        "Error parsing reply document".to_string()
                                    }
                                } else {
                                    "Error parsing reply content".to_string()
                                }
                            } else {
                                "Error fetching reply content".to_string()
                            }
                        } else {
                            "Error fetching reply content".to_string()
                        };

                        replies_html.push_str(&format!(
                            "<div class=\"reply\" style=\"margin: 15px 0; padding: 15px; background-color: #f8f9fa; border-left: 4px solid #007bff; border-radius: 5px;\">
                                <div class=\"reply-meta\" style=\"font-size: 0.9em; color: #666; margin-bottom: 10px;\">
                                    <strong>Reply by {reply_uploader}</strong> • {reply_created} • 👍 {reply_upvotes}
                                </div>
                                <div class=\"reply-content\">{reply_content}</div>
                            </div>"
                        ));
                    }
                    replies_html.push_str("</div>");
                }
            }
        }

        document_data.push((
            doc_id,
            revision,
            content_id,
            created_at,
            uploader_username.to_string(),
            format!("{html_content}{replies_html}"),
            upvote_count,
            tags,
            authors,
        ));
    }

    if document_data.is_empty() {
        return Err("No valid documents could be fetched".into());
    }

    // Sort by revision DESC to get latest first
    document_data.sort_by(|a, b| b.1.cmp(&a.1));
    let latest_doc = &document_data[0];

    // Create revision navigation for sidebar and embedded document data
    let mut revision_links = String::new();
    let mut embedded_documents = String::new();

    if document_data.len() > 1 {
        for (
            i,
            (
                doc_id,
                doc_revision,
                content_id,
                doc_created,
                username,
                html_content,
                upvote_count,
                _,
                _,
            ),
        ) in document_data.iter().enumerate()
        {
            let is_current = i == 0; // First item is the latest
            let current_class = if is_current { " current" } else { "" };
            let display_style = if is_current { "block" } else { "none" };

            revision_links.push_str(&format!(
                r#"<div class="revision-item{current_class}" data-doc-id="{doc_id}" onclick="showRevision({doc_id})">
                    <div class="revision-link">Revision {doc_revision}</div>
                    <div class="revision-date">{doc_created}</div>
                    <div class="upvote-count">👍 {upvote_count}</div>
                    {current_indicator}
                </div>"#,
                current_class = current_class,
                doc_id = doc_id,
                doc_revision = doc_revision,
                doc_created = doc_created,
                upvote_count = upvote_count,
                current_indicator = if is_current { "<div style=\"font-size: 0.8em; color: #28a745; font-weight: bold;\">← Current</div>" } else { "" }
            ));

            // Add hidden div with document content
            embedded_documents.push_str(&format!(
                r#"<div id="document-{doc_id}" class="document-content" style="display: {display_style};" data-content-id="{content_id}" data-created="{doc_created}" data-uploader="{username}" data-revision="{doc_revision}" data-upvotes="{upvote_count}">
                    {html_content}
                </div>"#
            ));
        }
    } else {
        let (
            doc_id,
            doc_revision,
            content_id,
            doc_created,
            username,
            html_content,
            upvote_count,
            _tags,
            _authors,
        ) = &document_data[0];
        revision_links.push_str(&format!(
            r#"<div style="padding: 10px; color: #666; font-style: italic;">
                This post has only one revision.
                <div class="upvote-count" style="margin-top: 5px; color: #333;">👍 {upvote_count}</div>
            </div>"#
        ));

        embedded_documents.push_str(&format!(
            r#"<div id="document-{doc_id}" class="document-content" style="display: block;" data-content-id="{content_id}" data-created="{doc_created}" data-uploader="{username}" data-revision="{doc_revision}" data-upvotes="{upvote_count}">
                {html_content}
            </div>"#
        ));
    }

    let content_id_hex: String = latest_doc.2.encode_hex();
    let full_html = render_to_html(
        post_id,
        &content_id_hex,     // content_id
        &latest_doc.3,       // created_at
        &latest_doc.4,       // uploader_username
        &latest_doc.7,       // tags
        &latest_doc.8,       // authors
        &embedded_documents, // all documents embedded
        &revision_links,
    );

    // Write to a temporary file
    let temp_file = format!("/tmp/parcnet-post-{post_id}.html");
    std::fs::write(&temp_file, full_html)?;

    println!(
        "Opening post {post_id} with {} revisions in browser...",
        document_data.len()
    );
    println!("Latest uploader: {}", latest_doc.4);
    println!("Latest document ID: {}", latest_doc.2);
    println!("Latest revision: {}", latest_doc.1);
    println!("Latest created: {}", latest_doc.3);
    println!("Latest upvotes: {}", latest_doc.6);

    // Open in default browser
    if let Err(e) = webbrowser::open(&temp_file) {
        println!("Failed to open browser: {e}");
        println!("HTML file saved to: {temp_file}");
    } else {
        println!("Opened in browser: {temp_file}");
    }

    Ok(())
}

fn render_document_content_to_html(content: &DocumentContent) -> String {
    let mut html_parts = Vec::new();

    // Render message content with format detection
    if let Some(ref message) = content.message {
        let rendered_message = if is_markdown(message) {
            render_markdown_to_html(message)
        } else if is_html(message) {
            // Just use the HTML as-is (but could add sanitization here)
            message.clone()
        } else {
            // Treat as plain text and convert to HTML
            format!(
                "<pre style=\"white-space: pre-wrap; font-family: inherit;\">{}</pre>",
                escape_html(message)
            )
        };
        html_parts.push(rendered_message);
    }

    // Render file content
    if let Some(ref file) = content.file {
        let file_html = if file.mime_type.starts_with("image/") {
            // For images, create a data URL to display them
            let base64_content = base64_encode(&file.content);
            format!(
                r#"<div class="file-attachment" style="margin: 20px 0; padding: 15px; background-color: #f8f9fa; border-left: 4px solid #28a745; border-radius: 5px;">
                    <h4 style="margin: 0 0 10px 0; color: #155724;">📎 Image: {}</h4>
                    <p style="margin: 5px 0; color: #666;">
                        <strong>Type:</strong> {} | <strong>Size:</strong> {} bytes
                    </p>
                    <img src="data:{};base64,{}" alt="{}" style="max-width: 100%; height: auto; border: 1px solid #ddd; border-radius: 3px;" />
                </div>"#,
                file.name,
                file.mime_type,
                file.content.len(),
                file.mime_type,
                base64_content,
                file.name
            )
        } else if file.mime_type == "text/plain" {
            // Display plain text files
            let file_content = String::from_utf8_lossy(&file.content);
            format!(
                r#"<div class="file-attachment" style="margin: 20px 0; padding: 15px; background-color: #f8f9fa; border-left: 4px solid #28a745; border-radius: 5px;">
                    <h4 style="margin: 0 0 10px 0; color: #155724;">📎 Text File: {}</h4>
                    <p style="margin: 5px 0; color: #666;">
                        <strong>Type:</strong> {} | <strong>Size:</strong> {} bytes
                    </p>
                    <details><summary>View file content</summary>
                    <pre style="margin: 10px 0; padding: 10px; background-color: #fff; border: 1px solid #ddd; border-radius: 3px; overflow-x: auto; white-space: pre-wrap;"><code>{}</code></pre>
                    </details>
                </div>"#,
                file.name,
                file.mime_type,
                file.content.len(),
                escape_html(&file_content)
            )
        } else if file.mime_type == "text/markdown" {
            // Render markdown files
            let file_content = String::from_utf8_lossy(&file.content);
            let rendered_markdown = render_markdown_to_html(&file_content);
            format!(
                r#"<div class="file-attachment" style="margin: 20px 0; padding: 15px; background-color: #f8f9fa; border-left: 4px solid #28a745; border-radius: 5px;">
                    <h4 style="margin: 0 0 10px 0; color: #155724;">📎 Markdown File: {}</h4>
                    <p style="margin: 5px 0; color: #666;">
                        <strong>Type:</strong> {} | <strong>Size:</strong> {} bytes
                    </p>
                    <details open><summary>Rendered content</summary>
                    <div style="margin: 10px 0; padding: 10px; background-color: #fff; border: 1px solid #ddd; border-radius: 3px;">
                        {}
                    </div>
                    </details>
                    <details><summary>View raw markdown</summary>
                    <pre style="margin: 10px 0; padding: 10px; background-color: #fff; border: 1px solid #ddd; border-radius: 3px; overflow-x: auto; white-space: pre-wrap;"><code>{}</code></pre>
                    </details>
                </div>"#,
                file.name,
                file.mime_type,
                file.content.len(),
                rendered_markdown,
                escape_html(&file_content)
            )
        } else if file.mime_type == "application/json" {
            // Pretty-print JSON files
            let file_content = String::from_utf8_lossy(&file.content);
            let formatted_json = match serde_json::from_str::<serde_json::Value>(&file_content) {
                Ok(json) => {
                    serde_json::to_string_pretty(&json).unwrap_or_else(|_| file_content.to_string())
                }
                Err(_) => file_content.to_string(),
            };
            format!(
                r#"<div class="file-attachment" style="margin: 20px 0; padding: 15px; background-color: #f8f9fa; border-left: 4px solid #28a745; border-radius: 5px;">
                    <h4 style="margin: 0 0 10px 0; color: #155724;">📎 JSON File: {}</h4>
                    <p style="margin: 5px 0; color: #666;">
                        <strong>Type:</strong> {} | <strong>Size:</strong> {} bytes
                    </p>
                    <details><summary>View JSON content</summary>
                    <pre style="margin: 10px 0; padding: 10px; background-color: #fff; border: 1px solid #ddd; border-radius: 3px; overflow-x: auto; white-space: pre-wrap;"><code>{}</code></pre>
                    </details>
                </div>"#,
                file.name,
                file.mime_type,
                file.content.len(),
                escape_html(&formatted_json)
            )
        } else {
            // For other file types, just show metadata
            format!(
                r#"<div class="file-attachment" style="margin: 20px 0; padding: 15px; background-color: #f8f9fa; border-left: 4px solid #28a745; border-radius: 5px;">
                    <h4 style="margin: 0 0 10px 0; color: #155724;">📎 File: {}</h4>
                    <p style="margin: 5px 0; color: #666;">
                        <strong>Type:</strong> {} | <strong>Size:</strong> {} bytes
                    </p>
                    <p style="margin: 5px 0; color: #666;"><em>Preview not available for this file type</em></p>
                </div>"#,
                file.name,
                file.mime_type,
                file.content.len()
            )
        };
        html_parts.push(file_html);
    }

    // Render URL content
    if let Some(ref url) = content.url {
        let url_html = format!(
            r#"<div class="url-reference" style="margin: 20px 0; padding: 15px; background-color: #f8f9fa; border-left: 4px solid #007bff; border-radius: 5px;">
                <h4 style="margin: 0 0 10px 0; color: #004085;">🔗 Referenced URL</h4>
                <p style="margin: 0;"><a href="{url}" target="_blank" style="color: #007bff; text-decoration: none; font-weight: 500;">{url}</a></p>
            </div>"#
        );
        html_parts.push(url_html);
    }

    if html_parts.is_empty() {
        "<p>No content available</p>".to_string()
    } else {
        html_parts.join("\n")
    }
}

fn render_markdown_to_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_SMART_PUNCTUATION);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);

    let parser = Parser::new_ext(markdown, options);
    let mut events = Vec::new();
    let mut in_math = false;
    let mut math_content = String::new();

    // Process markdown with math support
    for event in parser {
        match event {
            Event::Text(text) => {
                let text_str = text.as_ref();
                if text_str.starts_with("$$") {
                    in_math = true;
                    math_content = text_str.to_string();
                } else if text_str.ends_with("$$") && in_math {
                    math_content.push_str(text_str);
                    events.push(Event::Html(math_content.clone().into()));
                    math_content.clear();
                    in_math = false;
                } else if in_math {
                    math_content.push_str(text_str);
                } else {
                    events.push(Event::Text(text));
                }
            }
            _ if in_math => continue,
            other => events.push(other),
        }
    }

    let mut html_output = String::new();
    html::push_html(&mut html_output, events.into_iter());
    html_output
}

fn is_markdown(text: &str) -> bool {
    // Simple heuristics to detect markdown
    text.contains("# ")
        || text.contains("## ")
        || text.contains("**")
        || text.contains("*")
        || text.contains("[")
        || text.contains("`")
        || text.contains("- ")
        || text.contains("1. ")
}

fn is_html(text: &str) -> bool {
    // Simple heuristics to detect HTML
    text.contains("<")
        && text.contains(">")
        && (text.contains("<p>")
            || text.contains("<div>")
            || text.contains("<h1>")
            || text.contains("<span>")
            || text.contains("<br>")
            || text.contains("</"))
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn base64_encode(data: &[u8]) -> String {
    // Simple base64 encoding (in a real implementation, you'd use a proper base64 crate)
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();

    for chunk in data.chunks(3) {
        let mut buf = [0u8; 3];
        for (i, &byte) in chunk.iter().enumerate() {
            buf[i] = byte;
        }

        let b0 = buf[0] as usize;
        let b1 = buf[1] as usize;
        let b2 = buf[2] as usize;

        result.push(ALPHABET[b0 >> 2] as char);
        result.push(ALPHABET[((b0 & 0x03) << 4) | (b1 >> 4)] as char);

        if chunk.len() > 1 {
            result.push(ALPHABET[((b1 & 0x0f) << 2) | (b2 >> 6)] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(ALPHABET[b2 & 0x3f] as char);
        } else {
            result.push('=');
        }
    }

    result
}
