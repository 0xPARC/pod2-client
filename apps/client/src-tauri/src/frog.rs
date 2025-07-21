use pod2::{
    backends::plonky2::{primitives::ec::schnorr::SecretKey, signedpod::Signer},
    frontend::{SignedPod, SignedPodBuilder},
    middleware::TypedValue,
};
use pod2_db::store::{self, create_space, space_exists, PodData};
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::Mutex;

use crate::{config::config, AppState};

fn server_url(path: &str) -> String {
    let domain = config().network.frogcrypto_server.clone();
    format!("{domain}/{path}")
}

fn connection_failed<T>(_: T) -> String {
    "failed to connect to server".to_string()
}

#[derive(Deserialize)]
struct Challenge {
    public_key: String,
    time: String,
}

#[derive(Deserialize)]
struct FrogResponse {
    pod: SignedPod,
    score: i64,
}

async fn process_challenge(client: &Client, private_key: SecretKey) -> Result<SignedPod, String> {
    let challenge_url = server_url("auth");
    let challenge: Challenge = client
        .get(&challenge_url)
        .send()
        .await
        .map_err(connection_failed)?
        .json()
        .await
        .map_err(connection_failed)?;
    let mut builder = SignedPodBuilder::new(&Default::default());
    builder.insert("public_key", challenge.public_key);
    builder.insert("time", challenge.time);
    let mut signer = Signer(private_key);
    builder
        .sign(&mut signer)
        .map_err(|_| "failed to sign pod".to_string())
}

async fn download_frog(client: &Client, private_key: SecretKey) -> Result<Response, String> {
    let pod = process_challenge(client, private_key).await?;
    let frog_url = server_url("frog");
    client
        .post(&frog_url)
        .json(&pod)
        .send()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn request_frog(state: State<'_, Mutex<AppState>>) -> Result<i64, String> {
    let client = Client::new();
    let mut app_state = state.lock().await;
    let private_key = crate::get_private_key(&app_state.db).await?;
    let frog_response: FrogResponse = download_frog(&client, private_key)
        .await?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    if !space_exists(&app_state.db, "frogs")
        .await
        .map_err(|e| e.to_string())?
    {
        create_space(&app_state.db, "frogs")
            .await
            .map_err(|e| e.to_string())?;
    }
    let name = match frog_response
        .pod
        .get("name")
        .map(pod2::middleware::Value::typed)
    {
        Some(TypedValue::String(s)) => Some(s.clone()),
        _ => None,
    };
    store::import_pod(
        &app_state.db,
        &PodData::Signed(frog_response.pod.into()),
        name.as_deref(),
        "frogs",
    )
    .await
    .map_err(|e| format!("Failed to save POD: {e}"))?;
    app_state.trigger_state_sync().await?;
    Ok(frog_response.score)
}

#[tauri::command]
pub async fn request_score(state: State<'_, Mutex<AppState>>) -> Result<serde_json::Value, String> {
    let client = Client::new();
    let app_state = state.lock().await;
    let private_key = crate::get_private_key(&app_state.db).await?;
    let pod = process_challenge(&client, private_key).await?;
    let score_url = server_url("score");
    client
        .post(&score_url)
        .json(&pod)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

#[derive(Serialize, Deserialize)]
pub struct LeaderboardRow {
    username: String,
    score: i64,
}

#[tauri::command]
pub async fn request_leaderboard(
    _state: State<'_, Mutex<AppState>>,
) -> Result<Vec<LeaderboardRow>, String> {
    let client = Client::new();
    client
        .get(server_url("leaderboard"))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

#[ignore]
#[tokio::test]
async fn test_request_frog() -> Result<(), String> {
    let client = Client::new();
    let private_key = SecretKey::new_rand();
    let response = download_frog(&client, private_key).await?;
    println!("{}", response.text().await.unwrap());
    Ok(())
}
