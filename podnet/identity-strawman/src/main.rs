use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
};
use pod_utils::ValueExt;
use pod2::backends::plonky2::{
    primitives::ec::{curve::Point as PublicKey, schnorr::SecretKey},
    signedpod::Signer,
};
use pod2::frontend::{SignedPod, SignedPodBuilder};
use pod2::middleware::Params;
use rand::Rng;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// Identity server state
pub struct IdentityServerState {
    pub server_id: String,
    pub server_secret_key: Arc<SecretKey>,
    pub server_public_key: PublicKey,
    pub db_conn: Arc<Mutex<Connection>>,
}

impl Clone for IdentityServerState {
    fn clone(&self) -> Self {
        Self {
            server_id: self.server_id.clone(),
            server_secret_key: Arc::clone(&self.server_secret_key),
            server_public_key: self.server_public_key,
            db_conn: Arc::clone(&self.db_conn),
        }
    }
}

// User registration models (new challenge-response flow)
#[derive(Debug, Deserialize)]
pub struct UserChallengeRequest {
    pub username: String,
    pub user_public_key: PublicKey,
}

#[derive(Debug, Serialize)]
pub struct UserChallengeResponse {
    /// SignedPod containing challenge information from identity server:
    /// - challenge: String (random challenge value)
    /// - expires_at: String (ISO timestamp when challenge expires)
    /// - user_public_key: Point (user's public key from request)
    /// - username: String (username from request)
    /// - _signer: Point (identity server's public key, automatically added by SignedPod)
    pub challenge_pod: SignedPod,
}

#[derive(Debug, Deserialize)]
pub struct IdentityRequest {
    /// Identity request containing both identity server's challenge and user's response
    ///
    /// server_challenge_pod contains:
    /// - challenge: String (original challenge from identity server)
    /// - expires_at: String (expiration timestamp)
    /// - user_public_key: Point (user's public key)
    /// - username: String (username)
    /// - _signer: Point (identity server's public key)
    ///
    /// user_response_pod contains:
    /// - challenge: String (same challenge value, proving user received it)
    /// - username: String (confirming username)
    /// - _signer: Point (user's public key, proving control of private key)
    pub server_challenge_pod: SignedPod,
    pub user_response_pod: SignedPod,
}

#[derive(Debug, Serialize)]
pub struct IdentityResponse {
    /// SignedPod containing:
    /// - username: String (user's chosen username)
    /// - user_public_key: Point (user's public key)
    /// - identity_server_id: String (ID of this identity server)
    /// - _signer: Point (identity server's public key, automatically added by SignedPod)
    pub identity_pod: SignedPod,
}

#[derive(Debug, Serialize)]
pub struct ServerInfo {
    pub server_id: String,
    pub public_key: PublicKey,
}

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

// Keypair persistence models
#[derive(Debug, Serialize, Deserialize)]
pub struct IdentityServerKeypair {
    pub server_id: String,
    pub secret_key: String, // hex encoded
    pub public_key: PublicKey,
    pub created_at: String,
}

// Username lookup models
#[derive(Debug, Deserialize)]
pub struct UsernameLookupRequest {
    pub public_key: PublicKey,
}

#[derive(Debug, Serialize)]
pub struct UsernameLookupResponse {
    pub username: String,
}

// Root endpoint
async fn root(State(state): State<IdentityServerState>) -> Json<ServerInfo> {
    Json(ServerInfo {
        server_id: state.server_id.clone(),
        public_key: state.server_public_key,
    })
}

// Step 1: User requests a challenge for identity verification
async fn request_user_challenge(
    State(state): State<IdentityServerState>,
    Json(payload): Json<UserChallengeRequest>,
) -> Result<Json<UserChallengeResponse>, StatusCode> {
    tracing::info!(
        "User challenge requested for username: {}",
        payload.username
    );

    // Generate a secure random challenge
    let challenge: String = (0..32)
        .map(|_| rand::rng().random::<u8>())
        .map(|b| format!("{b:02x}"))
        .collect();

    // Create expiration timestamp (5 minutes from now)
    let expires_at = chrono::Utc::now() + chrono::Duration::minutes(5);
    let expires_at_str = expires_at.to_rfc3339();

    tracing::info!(
        "Generated challenge for user {}: {}",
        payload.username,
        challenge
    );
    tracing::info!("Challenge expires at: {}", expires_at_str);

    // Create challenge pod signed by identity server
    let params = Params::default();
    let mut challenge_builder = SignedPodBuilder::new(&params);

    challenge_builder.insert("challenge", challenge.as_str());
    challenge_builder.insert("expires_at", expires_at_str.as_str());
    challenge_builder.insert("user_public_key", payload.user_public_key);
    challenge_builder.insert("username", payload.username.as_str());

    // Sign with identity server's private key
    let mut identity_signer = Signer(SecretKey(state.server_secret_key.0.clone()));
    let challenge_pod = challenge_builder.sign(&mut identity_signer).map_err(|e| {
        tracing::error!("Failed to sign user challenge pod: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tracing::info!(
        "User challenge pod created and signed for: {}",
        payload.username
    );

    Ok(Json(UserChallengeResponse { challenge_pod }))
}

// Step 2: User submits both challenge pod and response, gets identity pod
async fn issue_identity(
    State(state): State<IdentityServerState>,
    Json(payload): Json<IdentityRequest>,
) -> Result<Json<IdentityResponse>, StatusCode> {
    tracing::info!("Processing identity request with challenge-response verification");

    // 1. Verify the identity server's challenge pod signature
    payload.server_challenge_pod.verify().map_err(|e| {
        tracing::error!("Failed to verify server challenge pod: {e}");
        StatusCode::BAD_REQUEST
    })?;

    // 2. Verify challenge pod was signed by this identity server
    let identity_server_public_key = state.server_public_key;
    let challenge_signer = payload
        .server_challenge_pod
        .get("_signer")
        .and_then(|v| v.as_public_key())
        .ok_or_else(|| {
            tracing::error!("Server challenge pod missing signer");
            StatusCode::BAD_REQUEST
        })?;

    if *challenge_signer != identity_server_public_key {
        tracing::error!("Server challenge pod not signed by this identity server");
        return Err(StatusCode::BAD_REQUEST);
    }

    // 3. Extract and verify challenge hasn't expired
    let expires_at_str = payload
        .server_challenge_pod
        .get("expires_at")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            tracing::error!("Server challenge pod missing expires_at");
            StatusCode::BAD_REQUEST
        })?;

    let expires_at = chrono::DateTime::parse_from_rfc3339(expires_at_str).map_err(|e| {
        tracing::error!("Invalid expires_at format: {e}");
        StatusCode::BAD_REQUEST
    })?;

    if chrono::Utc::now() > expires_at {
        tracing::error!("Challenge has expired");
        return Err(StatusCode::BAD_REQUEST);
    }

    // 4. Extract challenge and user info from challenge pod
    let challenge = payload
        .server_challenge_pod
        .get("challenge")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            tracing::error!("Server challenge pod missing challenge");
            StatusCode::BAD_REQUEST
        })?;

    let user_public_key = payload
        .server_challenge_pod
        .get("user_public_key")
        .and_then(|v| v.as_public_key())
        .ok_or_else(|| {
            tracing::error!("Server challenge pod missing user_public_key");
            StatusCode::BAD_REQUEST
        })?;

    let username = payload
        .server_challenge_pod
        .get("username")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            tracing::error!("Server challenge pod missing username");
            StatusCode::BAD_REQUEST
        })?;

    // 5. Verify user's response pod
    payload.user_response_pod.verify().map_err(|e| {
        tracing::error!("Failed to verify user response pod: {e}");
        StatusCode::BAD_REQUEST
    })?;

    // 6. Verify response pod signed by user
    let response_signer = payload
        .user_response_pod
        .get("_signer")
        .and_then(|v| v.as_public_key())
        .ok_or_else(|| {
            tracing::error!("User response pod missing signer");
            StatusCode::BAD_REQUEST
        })?;

    if *response_signer != *user_public_key {
        tracing::error!("User response pod not signed by expected user");
        return Err(StatusCode::BAD_REQUEST);
    }

    // 7. Verify response pod contains same challenge
    let response_challenge = payload
        .user_response_pod
        .get("challenge")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            tracing::error!("User response pod missing challenge");
            StatusCode::BAD_REQUEST
        })?;

    if response_challenge != challenge {
        tracing::error!("Challenge mismatch between server and user pods");
        return Err(StatusCode::BAD_REQUEST);
    }

    // 8. Verify response pod contains same username
    let response_username = payload
        .user_response_pod
        .get("username")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            tracing::error!("User response pod missing username");
            StatusCode::BAD_REQUEST
        })?;

    if response_username != username {
        tracing::error!("Username mismatch between challenge and response pods");
        return Err(StatusCode::BAD_REQUEST);
    }

    tracing::info!("✓ All verifications passed for user: {}", username);

    // Create identity pod using SignedPodBuilder
    let params = Params::default();
    let mut identity_builder = SignedPodBuilder::new(&params);

    identity_builder.insert("username", username);
    identity_builder.insert("user_public_key", *user_public_key);
    identity_builder.insert("identity_server_id", state.server_id.as_str());
    identity_builder.insert("issued_at", chrono::Utc::now().to_rfc3339().as_str());

    // Sign the identity pod with the identity server's key
    let mut server_signer = Signer(SecretKey(state.server_secret_key.0.clone()));
    let identity_pod = identity_builder.sign(&mut server_signer).map_err(|e| {
        tracing::error!("Failed to sign identity pod: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tracing::info!("Identity pod issued for user: {}", username);

    // Store username-public key mapping in database
    {
        let conn = state.db_conn.lock().unwrap();
        if let Err(e) = insert_user_mapping(&conn, user_public_key, username) {
            tracing::error!("Failed to store username mapping: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    Ok(Json(IdentityResponse { identity_pod }))
}

// Username lookup handler
async fn lookup_username_by_public_key(
    State(state): State<IdentityServerState>,
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

// Register this identity server with the podnet-server
async fn register_with_podnet_server(
    server_id: &str,
    secret_key: &SecretKey,
    podnet_server_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Registering with podnet-server at: {}", podnet_server_url);

    let public_key = secret_key.public_key();
    let client = reqwest::Client::new();

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
        return Err(format!("Failed to get challenge. Status: {status} - {error_text}").into());
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
        .ok_or("Server challenge pod missing challenge")?;

    tracing::info!("Challenge received: {}", challenge);

    // Step 3: Create identity server's response pod
    let params = Params::default();
    let mut response_builder = SignedPodBuilder::new(&params);

    response_builder.insert("challenge", challenge);
    response_builder.insert("server_id", server_id);

    // Sign the response with identity server's private key
    let mut identity_signer = Signer(SecretKey(secret_key.0.clone()));
    let identity_response_pod = response_builder.sign(&mut identity_signer)?;

    tracing::info!("✓ Created identity server response pod");

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
        tracing::info!("✓ Successfully registered with podnet-server!");
        tracing::info!("PodNet Server Public Key: {}", server_info.public_key);
        Ok(())
    } else {
        let status = registration_response.status();
        let error_text = registration_response.text().await?;

        if status == reqwest::StatusCode::CONFLICT {
            tracing::info!("✓ Identity server already registered with podnet-server");
            Ok(())
        } else {
            tracing::error!("Failed to register with podnet-server. Status: {}", status);
            tracing::error!("Error: {}", error_text);
            Err(format!("Registration failed: {status} - {error_text}").into())
        }
    }
}

// Keypair management functions
fn load_or_create_keypair(keypair_file: &str) -> anyhow::Result<(String, SecretKey, PublicKey)> {
    let server_id = "strawman-identity-server".to_string();

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

// Database initialization function
fn initialize_database(db_path: &str) -> anyhow::Result<Connection> {
    tracing::info!("Initializing database at: {}", db_path);

    let conn = Connection::open(db_path)?;

    // Create the users table if it doesn't exist
    conn.execute(
        "CREATE TABLE IF NOT EXISTS users (
            public_key_json TEXT PRIMARY KEY,
            username TEXT NOT NULL,
            issued_at TEXT NOT NULL
        )",
        [],
    )?;

    tracing::info!("✓ Database initialized successfully");
    Ok(conn)
}

// Database operations
fn insert_user_mapping(
    conn: &Connection,
    public_key: &PublicKey,
    username: &str,
) -> anyhow::Result<()> {
    let public_key_json = serde_json::to_string(public_key)?;
    let issued_at = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT OR REPLACE INTO users (public_key_json, username, issued_at) VALUES (?1, ?2, ?3)",
        params![public_key_json, username, issued_at],
    )?;

    tracing::info!(
        "✓ Stored username mapping: {} -> {}",
        username,
        public_key_json
    );
    Ok(())
}

fn get_username_by_public_key(
    conn: &Connection,
    public_key: &PublicKey,
) -> anyhow::Result<Option<String>> {
    let public_key_json = serde_json::to_string(public_key)?;

    let mut stmt = conn.prepare("SELECT username FROM users WHERE public_key_json = ?1")?;
    let mut rows = stmt.query(params![public_key_json])?;

    if let Some(row) = rows.next()? {
        let username: String = row.get(0)?;
        Ok(Some(username))
    } else {
        Ok(None)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "podnet_ident_strawman=debug,tower_http=debug,axum::routing=trace".into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting PodNet Identity Server (Strawman Implementation)...");

    // Load or create server keypair
    let keypair_file = std::env::var("IDENTITY_KEYPAIR_FILE")
        .unwrap_or_else(|_| "identity-server-keypair.json".to_string());
    tracing::info!("Using keypair file: {}", keypair_file);

    let (server_id, server_secret_key, server_public_key) = load_or_create_keypair(&keypair_file)?;

    tracing::info!("Identity Server ID: {}", server_id);
    tracing::info!("Server Public Key: {}", server_public_key);

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
    let db_path =
        std::env::var("IDENTITY_DATABASE_PATH").unwrap_or_else(|_| "identity-users.db".to_string());
    tracing::info!("Using database file: {}", db_path);

    let db_conn = initialize_database(&db_path)?;
    let db_conn = Arc::new(Mutex::new(db_conn));

    let state = IdentityServerState {
        server_id: server_id.clone(),
        server_secret_key: Arc::new(server_secret_key),
        server_public_key,
        db_conn,
    };

    let app = Router::new()
        .route("/", get(root))
        .route("/user/challenge", post(request_user_challenge))
        .route("/identity", post(issue_identity))
        .route("/lookup", get(lookup_username_by_public_key))
        .layer(CorsLayer::permissive())
        .with_state(state);

    tracing::info!("Binding to 0.0.0.0:3001...");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await?;
    tracing::info!("Identity server running on http://localhost:3001");
    tracing::info!("Available endpoints:");
    tracing::info!("  GET  /                - Server info");
    tracing::info!("  POST /user/challenge  - Request challenge for user identity");
    tracing::info!("  POST /identity        - Submit challenge response, get identity pod");
    tracing::info!(
        "  GET  /lookup          - Look up username by public key (query param: public_key)"
    );

    axum::serve(listener, app).await?;
    Ok(())
}
