use pod2::{
    backends::plonky2::signedpod::Signer,
    frontend::{SignedPod, SignedPodBuilder},
    middleware::TypedValue,
};
use pod2_db::store::{self, create_space, space_exists, PodData};
use reqwest::Client;
use serde::Deserialize;
use tauri::State;
use tokio::sync::Mutex;

use crate::AppState;

const SERVER_URL: &str = "https://frog-server-q36c.onrender.com";

fn server_url(path: &str) -> String {
    let domain = std::env::var("FROG_SERVER_URL").unwrap_or_else(|_| SERVER_URL.to_string());
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

#[tauri::command]
pub async fn request_frog(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let client = Client::new();
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
    let mut app_state = state.lock().await;
    let private_key = crate::get_private_key(&app_state.db).await?;
    let mut signer = Signer(private_key);
    let pod = builder
        .sign(&mut signer)
        .map_err(|_| "failed to sign pod")?;
    let frog_url = server_url("frog");
    let frog_pod: SignedPod = client
        .post(&frog_url)
        .json(&pod)
        .send()
        .await
        .map_err(|e| e.to_string())?
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
    let name = match frog_pod.get("name").map(pod2::middleware::Value::typed) {
        Some(TypedValue::String(s)) => Some(s.clone()),
        _ => None,
    };
    store::import_pod(
        &app_state.db,
        &PodData::Signed(frog_pod.into()),
        name.as_deref(),
        "frogs",
    )
    .await
    .map_err(|e| format!("Failed to save POD: {e}"))?;
    app_state.trigger_state_sync().await
}
