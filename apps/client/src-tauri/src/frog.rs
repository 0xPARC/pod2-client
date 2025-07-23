use std::sync::{Arc, Condvar};

use pod2::{
    backends::plonky2::{
        basetypes::F,
        primitives::ec::{curve::Point, schnorr::SecretKey},
        signedpod::Signer,
    },
    frontend::{SignedPod, SignedPodBuilder},
    middleware::{hash_fields, hash_values, Hash, HashOut, RawValue, TypedValue, Value},
};
use pod2_db::store::{self, create_space, get_default_private_key, space_exists, PodData};
use rand::{rngs::OsRng, Rng};
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Listener, Manager, Runtime, State};
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
    let signer = Signer(private_key);
    builder
        .sign(&signer)
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
        &PodData::Signed(Box::new(frog_response.pod.into())),
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

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
struct WorkerData {
    biome: i64,
    public_key: Point,
}

impl WorkerData {
    fn salt(&self) -> RawValue {
        let salt_hash = hash_values(&[Value::from(self.public_key), Value::from(self.biome)]);
        RawValue::from(salt_hash)
    }
}

struct WorkerSync {
    state: std::sync::Mutex<Option<WorkerData>>,
    cond: Condvar,
}

impl WorkerSync {
    fn new() -> Self {
        Self {
            state: std::sync::Mutex::new(None),
            cond: Condvar::new(),
        }
    }
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

struct FrogSearch {
    salt: RawValue,
    data: RawValue,
}

impl FrogSearch {
    fn salt(&self) -> &RawValue {
        &self.salt
    }

    fn search(&mut self) -> Option<(RawValue, RawValue)> {
        for _ in 0..MAX_TRIES_BEFORE_POLLING {
            //let hash = hash_values(&[self.salt.clone(), self.data.into()]);
            let hash = hash_fields(&[
                self.salt.0[0],
                self.salt.0[1],
                self.salt.0[2],
                self.salt.0[3],
                self.data.0[0],
                self.data.0[1],
                self.data.0[2],
                self.data.0[3],
            ]);
            if hash.0[0].0 & 0xFFFFFFC000000000 == 0 {
                return Some((self.data, RawValue::from(hash)));
            }
            self.data.0[3].0 += 1;
        }
        None
    }
}

fn search_seed() -> RawValue {
    let seed: i64 = OsRng.gen();
    RawValue::from(&TypedValue::Int(seed))
}

pub(crate) fn setup_background_thread<R: Runtime>(app_handle: AppHandle<R>) {
    let sync_ui = Arc::new(WorkerSync::new());
    let sync_background = sync_ui.clone();
    let app_handle_clone = app_handle.clone();
    app_handle.listen("toggle-mining", move |event| {
        let app_handle_clone2 = app_handle_clone.clone();
        let sync_ui_clone = sync_ui.clone();
        tauri::async_runtime::spawn(async move {
            let biome: i64 = if event.payload() == "true" { 1 } else { 0 };
            let private_key = if biome != 0 {
                let state = app_handle_clone2.state::<Mutex<AppState>>();
                let app_state = state.lock().await;
                get_default_private_key(&app_state.db).await.ok()
            } else {
                None
            };
            let worker_data = private_key.map(|sk| WorkerData {
                biome,
                public_key: sk.public_key(),
            });
            log::error!("{:?}", worker_data);
            let mut worker_data_shared = sync_ui_clone.state.lock().unwrap();
            *worker_data_shared = worker_data;
            if worker_data.is_some() {
                sync_ui_clone.cond.notify_one();
            }
        });
    });
    std::thread::spawn(move || {
        let mut count = 0;
        let mut old_worker_data = None;
        let mut search = FrogSearch {
            salt: RawValue::default(),
            data: search_seed(),
        };
        loop {
            let mut worker_data_shared = sync_background.state.lock().unwrap();
            while worker_data_shared.is_none() {
                worker_data_shared = sync_background.cond.wait(worker_data_shared).unwrap();
            }
            let worker_data = *worker_data_shared;
            drop(worker_data_shared);
            if worker_data != old_worker_data {
                old_worker_data = worker_data;
                search.salt = worker_data.unwrap().salt();
            }
            if search.search().is_none() {
                if let Err(e) = app_handle.emit("frog-background", format!("test message {count}"))
                {
                    log::error!("{e}");
                } else {
                    count += 1;
                }
            }
        }
    });
}

const MAX_TRIES_BEFORE_POLLING: u64 = 20000;
