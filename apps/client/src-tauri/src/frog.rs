use std::sync::{Arc, Condvar, LazyLock};

use num::traits::Euclid;
use pod2::{
    backends::plonky2::{
        basetypes::F,
        primitives::{
            ec::{curve::Point, schnorr::SecretKey},
            merkletree::MerkleTree,
        },
        signedpod::Signer,
    },
    frontend::{SignedPod, SignedPodBuilder},
    middleware::{
        hash_fields, hash_str, hash_values, Hash, HashOut, Params, PodType, RawValue, TypedValue,
        Value, KEY_SIGNER, KEY_TYPE,
    },
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

struct FrogSearch {
    kvs: std::collections::HashMap<RawValue, RawValue>,
    max_depth: usize,
    nonce_key: RawValue,
}

const MAX_TRIES_BEFORE_POLLING: u64 = 1000;

impl FrogSearch {
    fn new(data: &WorkerData) -> Self {
        let mut kvs = std::collections::HashMap::new();
        let type_key: RawValue = hash_str(KEY_TYPE).into();
        let type_value: RawValue = (PodType::Signed as i64).into();
        let signer_key: RawValue = hash_str(KEY_SIGNER).into();
        let signer_value: RawValue = (&TypedValue::PublicKey(data.public_key)).into();
        let biome_key: RawValue = hash_str("biome").into();
        let biome_value: RawValue = data.biome.into();
        let nonce_key: RawValue = hash_str("nonce").into();
        let nonce_i64: i64 = OsRng.gen();
        let seed_value: RawValue = nonce_i64.into();
        kvs.insert(type_key, type_value);
        kvs.insert(signer_key, signer_value);
        kvs.insert(biome_key, biome_value);
        kvs.insert(nonce_key, seed_value);
        let max_depth = Params::default().max_depth_mt_containers;
        FrogSearch {
            kvs,
            max_depth,
            nonce_key,
        }
    }

    fn pod_hash(&self) -> Hash {
        MerkleTree::new(self.max_depth, &self.kvs).unwrap().root()
    }

    fn search_one(&mut self) -> bool {
        self.kvs.get_mut(&self.nonce_key).unwrap().0[3].0 += 1;
        let root = self.pod_hash();
        root.0[0].0 & 0xFFFFFC0000000000 == 0
    }

    fn search(&mut self) -> bool {
        for _ in 0..MAX_TRIES_BEFORE_POLLING {
            if self.search_one() {
                return true;
            }
        }
        return false;
    }
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
        let mut search = None;
        loop {
            let mut worker_data_shared = sync_background.state.lock().unwrap();
            while worker_data_shared.is_none() {
                worker_data_shared = sync_background.cond.wait(worker_data_shared).unwrap();
            }
            let worker_data = *worker_data_shared;
            drop(worker_data_shared);
            if worker_data != old_worker_data {
                old_worker_data = worker_data;
                search = worker_data.as_ref().map(FrogSearch::new);
            }
            if !search.as_mut().unwrap().search() {
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

struct FrogStats {
    frog_id: u64,
    jump: u64,
    speed: u64,
    intelligence: u64,
    beauty: u64,
    temperament: u64,
}

fn compute_frog_stats(biome: i64, id: Hash) -> FrogStats {
    match biome {
        _ => {
            let val = id.0[0].0;
            let (val, temperament) = val.div_rem_euclid(&7);
            let (val, beauty) = val.div_rem_euclid(&8);
            let (val, intelligence) = val.div_rem_euclid(&8);
            let (val, speed) = val.div_rem_euclid(&8);
            let (val, jump) = val.div_rem_euclid(&8);
            let frog_id = val % 40 + 1;
            FrogStats {
                frog_id,
                jump,
                speed,
                intelligence,
                beauty,
                temperament,
            }
        }
    }
}

#[cfg(test)]
mod test {
    use pod2::{
        backends::plonky2::{primitives::ec::schnorr::SecretKey, signedpod::Signer},
        frontend::SignedPodBuilder,
        middleware::{hash_str, Params, PodId, RawValue},
    };
    use reqwest::Client;

    use crate::frog::{download_frog, FrogSearch, WorkerData};

    #[test]
    fn test_pod_hash() {
        let sk = SecretKey::new_rand();
        let biome = 1;
        let search = FrogSearch::new(&WorkerData {
            public_key: sk.public_key(),
            biome,
        });
        let search_id = PodId(search.pod_hash());
        let mut builder = SignedPodBuilder::new(&Params::default());
        builder.insert("biome", biome);
        let nonce_key: RawValue = hash_str("nonce").into();
        builder.insert("nonce", *search.kvs.get(&nonce_key).unwrap());
        let pod = builder.sign(&Signer(sk)).unwrap();
        assert_eq!(search_id, pod.id())
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
}
