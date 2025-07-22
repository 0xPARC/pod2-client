use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::Json};
use pod_utils::ValueExt;
use podnet_models::{
    IdentityServerChallengeRequest, IdentityServerChallengeResponse, IdentityServerRegistration,
    ServerInfo,
};

pub async fn request_identity_challenge(
    State(_state): State<Arc<crate::AppState>>,
    Json(payload): Json<IdentityServerChallengeRequest>,
) -> Result<Json<IdentityServerChallengeResponse>, StatusCode> {
    use pod2::{
        backends::plonky2::signedpod::Signer, frontend::SignedPodBuilder, middleware::Params,
    };
    use rand::Rng;

    log::info!(
        "Challenge requested for identity server: {}",
        payload.server_id
    );

    // Generate a secure random challenge
    let challenge: String = (0..32)
        .map(|_| rand::rng().random::<u8>())
        .map(|b| format!("{b:02x}"))
        .collect();

    // Create expiration timestamp (5 minutes from now)
    let expires_at = chrono::Utc::now() + chrono::Duration::minutes(5);
    let expires_at_str = expires_at.to_rfc3339();

    log::info!(
        "Generated challenge for {}: {}",
        payload.server_id,
        challenge
    );
    log::info!("Challenge expires at: {expires_at_str}");

    // Create challenge pod signed by server
    let params = Params::default();
    let mut challenge_builder = SignedPodBuilder::new(&params);

    challenge_builder.insert("challenge", challenge.as_str());
    challenge_builder.insert("expires_at", expires_at_str.as_str());
    challenge_builder.insert("identity_server_public_key", payload.public_key);
    challenge_builder.insert("server_id", payload.server_id.as_str());

    // Sign with server's private key
    let server_secret_key = crate::pod::get_server_secret_key();
    let server_signer = Signer(pod2::backends::plonky2::primitives::ec::schnorr::SecretKey(
        server_secret_key.0.clone(),
    ));
    let challenge_pod = challenge_builder.sign(&server_signer).map_err(|e| {
        log::error!("Failed to sign challenge pod: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    log::info!(
        "Challenge pod created and signed for identity server: {}",
        payload.server_id
    );

    Ok(Json(IdentityServerChallengeResponse { challenge_pod }))
}

pub async fn register_identity_server(
    State(state): State<Arc<crate::AppState>>,
    Json(payload): Json<IdentityServerRegistration>,
) -> Result<Json<ServerInfo>, StatusCode> {
    log::info!("Processing identity server registration");

    // 1. Verify the server's challenge pod signature
    payload.server_challenge_pod.verify().map_err(|e| {
        log::error!("Failed to verify server challenge pod: {e}");
        StatusCode::BAD_REQUEST
    })?;

    // 2. Verify challenge pod was signed by this server
    let server_public_key = crate::pod::get_server_public_key();
    let challenge_signer = payload
        .server_challenge_pod
        .get("_signer")
        .and_then(|v| v.as_public_key())
        .ok_or_else(|| {
            log::error!("Server challenge pod missing signer");
            StatusCode::BAD_REQUEST
        })?;

    if *challenge_signer != server_public_key {
        log::error!("Server challenge pod not signed by this server");
        return Err(StatusCode::BAD_REQUEST);
    }

    // 3. Extract and verify challenge hasn't expired
    let expires_at_str = payload
        .server_challenge_pod
        .get("expires_at")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            log::error!("Server challenge pod missing expires_at");
            StatusCode::BAD_REQUEST
        })?;

    let expires_at = chrono::DateTime::parse_from_rfc3339(expires_at_str).map_err(|e| {
        log::error!("Invalid expires_at format: {e}");
        StatusCode::BAD_REQUEST
    })?;

    if chrono::Utc::now() > expires_at {
        log::error!("Challenge has expired");
        return Err(StatusCode::BAD_REQUEST);
    }

    // 4. Extract challenge and identity server info from challenge pod
    let challenge = payload
        .server_challenge_pod
        .get("challenge")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            log::error!("Server challenge pod missing challenge");
            StatusCode::BAD_REQUEST
        })?;

    let identity_server_public_key = payload
        .server_challenge_pod
        .get("identity_server_public_key")
        .and_then(|v| v.as_public_key())
        .ok_or_else(|| {
            log::error!("Server challenge pod missing identity_server_public_key");
            StatusCode::BAD_REQUEST
        })?;

    let server_id = payload
        .server_challenge_pod
        .get("server_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            log::error!("Server challenge pod missing server_id");
            StatusCode::BAD_REQUEST
        })?;

    // 5. Verify identity server's response pod
    payload.identity_response_pod.verify().map_err(|e| {
        log::error!("Failed to verify identity response pod: {e}");
        StatusCode::BAD_REQUEST
    })?;

    // 6. Verify response pod signed by identity server
    let response_signer = payload
        .identity_response_pod
        .get("_signer")
        .and_then(|v| v.as_public_key())
        .ok_or_else(|| {
            log::error!("Identity response pod missing signer");
            StatusCode::BAD_REQUEST
        })?;

    if *response_signer != *identity_server_public_key {
        log::error!("Identity response pod not signed by expected identity server");
        return Err(StatusCode::BAD_REQUEST);
    }

    // 7. Verify response pod contains same challenge
    let response_challenge = payload
        .identity_response_pod
        .get("challenge")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            log::error!("Identity response pod missing challenge");
            StatusCode::BAD_REQUEST
        })?;

    if response_challenge != challenge {
        log::error!("Challenge mismatch between server and identity server pods");
        return Err(StatusCode::BAD_REQUEST);
    }

    // 8. Verify response pod contains same server_id
    let response_server_id = payload
        .identity_response_pod
        .get("server_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            log::error!("Identity response pod missing server_id");
            StatusCode::BAD_REQUEST
        })?;

    if response_server_id != server_id {
        log::error!("Server ID mismatch between challenge and response pods");
        return Err(StatusCode::BAD_REQUEST);
    }

    log::info!("✓ All verifications passed for identity server: {server_id}");

    // Check if identity server already exists
    if let Ok(Some(_)) = state.db.get_identity_server_by_id(server_id) {
        log::warn!("Identity server {server_id} already exists");
        return Err(StatusCode::CONFLICT);
    }

    let pk_string = serde_json::to_string(&identity_server_public_key).map_err(|e| {
        log::error!("Unable to serialize identity server public key: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Store both the server's challenge pod and identity server's response pod
    let challenge_pod_string =
        serde_json::to_string(&payload.server_challenge_pod).map_err(|e| {
            log::error!("Unable to serialize challenge pod: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let identity_pod_string =
        serde_json::to_string(&payload.identity_response_pod).map_err(|e| {
            log::error!("Unable to serialize identity pod: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Create identity server
    state
        .db
        .create_identity_server(
            server_id,
            &pk_string,
            &challenge_pod_string,
            &identity_pod_string,
        )
        .map_err(|e| {
            log::error!("Failed to create identity server {server_id}: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    log::info!("Identity server {server_id} registered successfully");

    // Return server info
    let server_pk = crate::pod::get_server_public_key();
    Ok(Json(ServerInfo {
        public_key: server_pk,
    }))
}
