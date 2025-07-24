use std::{
    fs,
    sync::{Arc, Mutex},
};

use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
    response::{Json, Redirect},
    routing::{get, post},
};
use chrono::Utc;
use pod2::backends::plonky2::primitives::ec::{curve::Point as PublicKey, schnorr::SecretKey};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod database;
mod github;
mod identity;
mod registration;

use database::{
    delete_user_by_github_id, get_username_by_public_key, initialize_database, insert_user_mapping,
    user_exists_by_github_id,
};
use github::{GitHubOAuthClient, GitHubOAuthConfig, OAuthCallbackQuery, parse_oauth_state};
use identity::{
    IdentityResponse, ServerInfo, UsernameLookupRequest, UsernameLookupResponse,
    create_identity_pod,
};
use registration::register_with_podnet_server;

// Server state
pub struct GitHubIdentityServerState {
    pub server_id: String,
    pub server_secret_key: Arc<SecretKey>,
    pub server_public_key: PublicKey,
    pub db_conn: Arc<Mutex<Connection>>,
    pub oauth_client: GitHubOAuthClient,
}

impl Clone for GitHubIdentityServerState {
    fn clone(&self) -> Self {
        Self {
            server_id: self.server_id.clone(),
            server_secret_key: Arc::clone(&self.server_secret_key),
            server_public_key: self.server_public_key,
            db_conn: Arc::clone(&self.db_conn),
            oauth_client: GitHubOAuthClient::new(GitHubOAuthConfig {
                client_id: std::env::var("GITHUB_CLIENT_ID").expect("GITHUB_CLIENT_ID must be set"),
                client_secret: std::env::var("GITHUB_CLIENT_SECRET")
                    .expect("GITHUB_CLIENT_SECRET must be set"),
                redirect_uri: std::env::var("GITHUB_REDIRECT_URI")
                    .expect("GITHUB_REDIRECT_URI must be set"),
            })
            .expect("Failed to create OAuth client"),
        }
    }
}

// Request models
#[derive(Debug, Deserialize)]
pub struct AuthUrlRequest {
    pub public_key: PublicKey,
    pub username: String, // Full name provided by user
}

#[derive(Debug, Serialize)]
pub struct AuthUrlResponse {
    pub auth_url: String,
    pub state: String,
}

#[derive(Debug, Deserialize)]
pub struct IdentityRequest {
    pub code: String,
    pub state: String,
    pub username: String,            // Full name provided by user
    pub challenge_signature: String, // User signs challenge containing GitHub info + their name
}

// Keypair persistence models
#[derive(Debug, Serialize, Deserialize)]
pub struct IdentityServerKeypair {
    pub server_id: String,
    pub secret_key: String, // hex encoded
    pub public_key: PublicKey,
    pub created_at: String,
}

// Root endpoint
async fn root(State(state): State<GitHubIdentityServerState>) -> Json<ServerInfo> {
    Json(ServerInfo {
        server_id: state.server_id.clone(),
        public_key: state.server_public_key,
    })
}

// Step 1: Get GitHub OAuth authorization URL
async fn get_auth_url(
    State(state): State<GitHubIdentityServerState>,
    Json(payload): Json<AuthUrlRequest>,
) -> Result<Json<AuthUrlResponse>, StatusCode> {
    tracing::info!(
        "Authorization URL requested for user: {} with public key: {}",
        payload.username,
        payload.public_key
    );

    let (auth_url, csrf_token) = state
        .oauth_client
        .get_authorization_url(&payload.public_key)
        .map_err(|e| {
            tracing::error!("Failed to generate authorization URL: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    tracing::info!("Generated authorization URL for user: {}", payload.username);

    Ok(Json(AuthUrlResponse {
        auth_url: auth_url.to_string(),
        state: csrf_token.secret().clone(),
    }))
}

// Step 2: Handle OAuth callback (redirect endpoint)
async fn oauth_callback(Query(params): Query<OAuthCallbackQuery>) -> Result<Redirect, StatusCode> {
    tracing::info!("OAuth callback received with code: {}", params.code);

    // Redirect back to the client application with the authorization code
    // The client will handle completing the identity verification
    let redirect_url = format!(
        "/identity/complete?code={}&state={}",
        params.code, params.state
    );

    Ok(Redirect::to(&redirect_url))
}

// Step 2.5: Handle OAuth completion page
async fn oauth_complete_page(
    Query(params): Query<OAuthCallbackQuery>,
) -> Result<axum::response::Html<String>, StatusCode> {
    tracing::info!("OAuth completion page requested with code: {}", params.code);

    // Return a simple HTML page that displays the authorization code
    // The user can copy this code back to the client application
    let html = format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>GitHub OAuth Complete</title>
            <style>
                body {{ font-family: Arial, sans-serif; margin: 40px; }}
                .container {{ max-width: 600px; margin: 0 auto; }}
                .code {{ background: #f5f5f5; padding: 15px; border-radius: 5px; font-family: monospace; word-break: break-all; }}
                .copy-btn {{ background: #0366d6; color: white; border: none; padding: 10px 20px; border-radius: 5px; cursor: pointer; margin-top: 10px; }}
                .copy-btn:hover {{ background: #0256cc; }}
            </style>
        </head>
        <body>
            <div class="container">
                <h1>🎉 GitHub Authorization Complete!</h1>
                <p>Your GitHub authorization was successful. Please copy the authorization code below and paste it into the POD2 client:</p>
                
                <div class="code" id="authCode">{}</div>
                
                <button class="copy-btn" onclick="copyCode()">📋 Copy Authorization Code</button>
                
                <p><small>You can now return to the POD2 client and paste this code to complete your identity setup.</small></p>
            </div>
            
            <script>
                function copyCode() {{
                    const code = document.getElementById('authCode').textContent;
                    navigator.clipboard.writeText(code).then(function() {{
                        const btn = document.querySelector('.copy-btn');
                        btn.textContent = '✅ Copied!';
                        setTimeout(() => {{
                            btn.textContent = '📋 Copy Authorization Code';
                        }}, 2000);
                    }});
                }}
            </script>
        </body>
        </html>
        "#,
        params.code
    );

    Ok(axum::response::Html(html))
}

// Step 3: Complete identity verification and issue POD
async fn issue_identity(
    State(state): State<GitHubIdentityServerState>,
    Json(payload): Json<IdentityRequest>,
) -> Result<Json<IdentityResponse>, StatusCode> {
    tracing::info!("Processing GitHub identity request");

    // Parse the public key from state
    let public_key = parse_oauth_state(&payload.state).map_err(|e| {
        tracing::error!("Failed to parse OAuth state: {}", e);
        StatusCode::BAD_REQUEST
    })?;

    // Exchange authorization code for access token
    let access_token = state
        .oauth_client
        .exchange_code(oauth2::AuthorizationCode::new(payload.code))
        .await
        .map_err(|e| {
            tracing::error!("Failed to exchange OAuth code: {}", e);
            StatusCode::BAD_REQUEST
        })?;

    // Get GitHub user info
    let github_user = state
        .oauth_client
        .get_user_info(&access_token)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get GitHub user info: {}", e);
            StatusCode::BAD_REQUEST
        })?;

    // Check if this GitHub user already has an identity and remove it if so
    {
        let conn = state.db_conn.lock().unwrap();
        if user_exists_by_github_id(&conn, github_user.id).map_err(|e| {
            tracing::error!("Database error checking GitHub user: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })? {
            tracing::info!(
                "GitHub user {} already has an identity, removing old record",
                github_user.login
            );
            delete_user_by_github_id(&conn, github_user.id).map_err(|e| {
                tracing::error!("Failed to delete existing GitHub user record: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        }
    }

    // Get SSH keys from GitHub
    let github_public_keys = state
        .oauth_client
        .get_ssh_keys(&github_user.login)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get GitHub SSH keys: {}", e);
            StatusCode::BAD_REQUEST
        })?;

    tracing::info!(
        "Retrieved {} SSH keys for GitHub user: {}",
        github_public_keys.len(),
        github_user.login
    );

    // TODO: Verify challenge signature from user
    // For now, we'll proceed without signature verification
    // In production, you'd want to verify that the user signed a challenge
    // containing their GitHub info and provided username

    let oauth_verified_at = Utc::now();

    // Create identity POD
    let identity_pod = create_identity_pod(
        &state.server_id,
        &state.server_secret_key,
        &public_key,
        &payload.username,
        &github_user,
        &github_public_keys,
        oauth_verified_at,
    )
    .map_err(|e| {
        tracing::error!("Failed to create identity POD: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Store user mapping in database
    {
        let conn = state.db_conn.lock().unwrap();
        insert_user_mapping(
            &conn,
            &public_key,
            &payload.username,
            &github_user.login,
            github_user.id,
            &github_public_keys,
            oauth_verified_at,
        )
        .map_err(|e| {
            tracing::error!("Failed to store user mapping: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    }

    tracing::info!(
        "✓ GitHub identity POD issued for user: {} (GitHub: {})",
        payload.username,
        github_user.login
    );

    Ok(Json(IdentityResponse { identity_pod }))
}

// Username lookup handler (for compatibility)
async fn lookup_username_by_public_key(
    State(state): State<GitHubIdentityServerState>,
    Query(params): Query<UsernameLookupRequest>,
) -> Result<Json<UsernameLookupResponse>, StatusCode> {
    tracing::info!("Looking up username for public key: {}", params.public_key);

    let conn = state.db_conn.lock().unwrap();

    match get_username_by_public_key(&conn, &params.public_key) {
        Ok(Some(username)) => {
            tracing::info!("✓ Found username: {}", username);
            Ok(Json(UsernameLookupResponse { username }))
        }
        Ok(None) => {
            tracing::info!("Username not found for public key: {}", params.public_key);
            Err(StatusCode::NOT_FOUND)
        }
        Err(e) => {
            tracing::error!("Database error during username lookup: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Keypair management functions
fn load_or_create_keypair(keypair_file: &str) -> anyhow::Result<(String, SecretKey, PublicKey)> {
    let server_id = "github-identity-server".to_string();

    if fs::metadata(keypair_file).is_ok() {
        tracing::info!("Loading existing keypair from: {}", keypair_file);
        let keypair_json = fs::read_to_string(keypair_file)?;
        let keypair: IdentityServerKeypair = serde_json::from_str(&keypair_json)?;

        // Verify server_id matches
        if keypair.server_id != server_id {
            return Err(anyhow::anyhow!(
                "Keypair server_id mismatch: expected {}, found {}",
                server_id,
                keypair.server_id
            ));
        }

        // Decode secret key
        let secret_key_bytes = hex::decode(&keypair.secret_key)?;
        let secret_key_bigint = num_bigint::BigUint::from_bytes_le(&secret_key_bytes);
        let secret_key = SecretKey(secret_key_bigint);

        // Verify public key matches
        let expected_public_key = secret_key.public_key();
        if expected_public_key != keypair.public_key {
            return Err(anyhow::anyhow!("Keypair public key mismatch"));
        }

        tracing::info!("✓ Keypair loaded successfully");
        tracing::info!("Created at: {}", keypair.created_at);

        Ok((server_id, secret_key, keypair.public_key))
    } else {
        tracing::info!("Creating new keypair and saving to: {}", keypair_file);

        // Generate new keypair
        let secret_key = SecretKey::new_rand();
        let public_key = secret_key.public_key();

        // Save keypair to file
        let keypair = IdentityServerKeypair {
            server_id: server_id.clone(),
            secret_key: hex::encode(secret_key.0.to_bytes_le()),
            public_key,
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        let keypair_json = serde_json::to_string_pretty(&keypair)?;
        fs::write(keypair_file, keypair_json)?;

        tracing::info!("✓ New keypair created and saved");

        Ok((server_id, secret_key, public_key))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "identity_github=debug,tower_http=debug,axum::routing=trace".into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting PodNet GitHub Identity Server...");

    // Load environment variables
    let github_client_id = std::env::var("GITHUB_CLIENT_ID")
        .expect("GITHUB_CLIENT_ID environment variable must be set");
    let github_client_secret = std::env::var("GITHUB_CLIENT_SECRET")
        .expect("GITHUB_CLIENT_SECRET environment variable must be set");
    let github_redirect_uri = std::env::var("GITHUB_REDIRECT_URI")
        .expect("GITHUB_REDIRECT_URI environment variable must be set");

    tracing::info!("GitHub OAuth Client ID: {}", github_client_id);
    tracing::info!("GitHub Redirect URI: {}", github_redirect_uri);

    // Load or create server keypair
    let keypair_file = std::env::var("IDENTITY_KEYPAIR_FILE")
        .unwrap_or_else(|_| "github-identity-server-keypair.json".to_string());
    tracing::info!("Using keypair file: {}", keypair_file);

    let (server_id, server_secret_key, server_public_key) = load_or_create_keypair(&keypair_file)?;

    tracing::info!("GitHub Identity Server ID: {}", server_id);
    tracing::info!("Server Public Key: {}", server_public_key);

    // Initialize OAuth client
    let oauth_config = GitHubOAuthConfig {
        client_id: github_client_id,
        client_secret: github_client_secret,
        redirect_uri: github_redirect_uri,
    };
    let oauth_client = GitHubOAuthClient::new(oauth_config)?;

    // Attempt to register with podnet-server
    let podnet_server_url =
        std::env::var("PODNET_SERVER_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());

    tracing::info!("Attempting to register with podnet-server...");
    if let Err(e) =
        register_with_podnet_server(&server_id, &server_secret_key, &podnet_server_url).await
    {
        tracing::warn!("Failed to register with podnet-server: {}", e);
        tracing::warn!("Identity server will continue running, but won't be registered.");
        tracing::warn!("Issued identity pods may not be accepted by podnet-server.");
    }

    // Initialize database
    let db_path = std::env::var("IDENTITY_DATABASE_PATH")
        .unwrap_or_else(|_| "github-identity-users.db".to_string());
    tracing::info!("Using database file: {}", db_path);

    let db_conn = initialize_database(&db_path)?;
    let db_conn = Arc::new(Mutex::new(db_conn));

    let state = GitHubIdentityServerState {
        server_id: server_id.clone(),
        server_secret_key: Arc::new(server_secret_key),
        server_public_key,
        db_conn,
        oauth_client,
    };

    let app = Router::new()
        .route("/", get(root))
        .route("/auth/github", post(get_auth_url))
        .route("/auth/github/callback", get(oauth_callback))
        .route("/identity/complete", get(oauth_complete_page))
        .route("/identity", post(issue_identity))
        .route("/lookup", get(lookup_username_by_public_key))
        .layer(CorsLayer::permissive())
        .with_state(state);

    // Configure server port
    let port = std::env::var("GITHUB_IDENTITY_PORT")
        .unwrap_or_else(|_| "3001".to_string())
        .parse::<u16>()
        .unwrap_or_else(|_| {
            tracing::warn!("Invalid GITHUB_IDENTITY_PORT, using default 3001");
            3001
        });
    
    let bind_addr = format!("0.0.0.0:{}", port);
    tracing::info!("Binding to {}...", bind_addr);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("GitHub Identity server running on http://localhost:{}", port);
    tracing::info!("Available endpoints:");
    tracing::info!("  GET  /                      - Server info");
    tracing::info!("  POST /auth/github           - Get GitHub OAuth authorization URL");
    tracing::info!("  GET  /auth/github/callback  - Handle OAuth callback");
    tracing::info!("  GET  /identity/complete     - OAuth completion page with authorization code");
    tracing::info!("  POST /identity              - Complete identity verification and get POD");
    tracing::info!("  GET  /lookup                - Look up username by public key");

    axum::serve(listener, app).await?;
    Ok(())
}
