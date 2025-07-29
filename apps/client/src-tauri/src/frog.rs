use std::sync::{Arc, Condvar};

use num::traits::Euclid;
use pod2::{
    backends::plonky2::{
        primitives::{ec::schnorr::SecretKey, merkletree::MerkleTree},
        signedpod::Signer,
    },
    frontend::{SerializedSignedPod, SignedPod, SignedPodBuilder},
    middleware::{
        hash_str, Hash, Params, PodId, PodType, RawValue, TypedValue, Value, KEY_SIGNER, KEY_TYPE,
    },
};
use pod2_db::{
    store::{self, create_space, get_default_private_key, space_exists, PodData, PodInfo},
    Db,
};
use rand::{rngs::OsRng, Rng};
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Listener, Manager, State};
use tokio::sync::Mutex;

use crate::{config::config, AppState};

fn server_url(path: &str) -> String {
    let domain = &config().network.frogcrypto_server;
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

#[derive(Serialize)]
pub struct FrogDesc {
    frog_id: i64,
    name: String,
    description: String,
    image_url: String,
}

#[derive(Serialize)]
pub struct FrogData {
    #[serde(flatten)]
    desc: FrogDesc,
    #[serde(flatten)]
    stats: FrogStats,
}

#[derive(Serialize)]
pub struct FrogPod {
    pod_id: PodId,
    data: Option<FrogData>,
}

#[derive(Serialize)]
pub struct FrogedexEntry {
    frog_id: i64,
    rarity: i64,
    name: String,
    image_url: String,
}

const FROG_RARITIES: [i64; 80] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 4, 3,
    3, 3, 4, 3, 3, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
];

trait AsTyped {
    fn as_str(&self) -> Option<&str>;
    fn as_int(&self) -> Option<i64>;
}

impl AsTyped for Value {
    fn as_str(&self) -> Option<&str> {
        match self.typed() {
            TypedValue::String(s) => Some(s),
            _ => None,
        }
    }

    fn as_int(&self) -> Option<i64> {
        match self.typed() {
            TypedValue::Int(i) => Some(*i),
            _ => None,
        }
    }
}

fn frog_data_for(pod: &SignedPod, desc: &SignedPod) -> Option<FrogData> {
    let frog_id = desc.get("frog_id")?.as_int()?;
    let name = desc.get("name")?.as_str()?.to_owned();
    let description = desc.get("description")?.as_str()?.to_owned();
    let image_url = desc.get("image_url")?.as_str()?.to_owned();
    let desc = FrogDesc {
        frog_id,
        name,
        description,
        image_url,
    };
    let stats = compute_frog_stats(frog_id, pod.id().0);
    Some(FrogData { desc, stats })
}

#[tauri::command]
pub async fn list_frogs(state: State<'_, Mutex<AppState>>) -> Result<Vec<FrogPod>, String> {
    let app_state = state.lock().await;
    let frog_pods = frog_pods(&app_state.db).await?;
    let frog_descs = description_pods(&app_state.db).await?;
    let frogs = frog_pods
        .into_iter()
        .map(|pod| {
            let desc = description_for(&pod, &frog_descs);
            let data = desc.and_then(|d| frog_data_for(&pod, d));
            FrogPod {
                pod_id: pod.id(),
                data,
            }
        })
        .collect();
    Ok(frogs)
}

fn frogedex_data_for(desc: &SignedPod) -> Option<(i64, String, String)> {
    Some((
        desc.get("frog_id")?.as_int()?,
        desc.get("name")?.as_str()?.to_owned(),
        desc.get("image_url")?.as_str()?.to_owned(),
    ))
}

#[tauri::command]
pub async fn get_frogedex(state: State<'_, Mutex<AppState>>) -> Result<Vec<FrogedexEntry>, String> {
    let app_state = state.lock().await;
    let frog_descs = description_pods(&app_state.db).await?;
    let mut entries: Vec<_> = FROG_RARITIES
        .iter()
        .enumerate()
        .map(|(n, &rarity)| FrogedexEntry {
            frog_id: (n + 1) as i64,
            rarity,
            name: "???".to_string(),
            image_url: "https://frogcrypto.vercel.app/images/pixel_frog.png".to_string(),
        })
        .collect();
    for desc in frog_descs {
        if let Some((frog_id, name, image_url)) = frogedex_data_for(&desc) {
            if (1..=80).contains(&frog_id) {
                let index = (frog_id - 1) as usize;
                entries[index].name = name;
                entries[index].image_url = image_url;
            }
        }
    }
    Ok(entries)
}

async fn register_pod(app_state: &mut AppState, pod: SignedPod, space: &str) -> Result<(), String> {
    if !space_exists(&app_state.db, space)
        .await
        .map_err(|e| e.to_string())?
    {
        create_space(&app_state.db, space)
            .await
            .map_err(|e| e.to_string())?;
    }
    store::import_pod(
        &app_state.db,
        &PodData::Signed(Box::new(pod.into())),
        None,
        space,
    )
    .await
    .map_err(|e| e.to_string())?;
    app_state.trigger_state_sync().await?;
    app_state
        .app_handle
        .emit("refresh-frogs", ())
        .map_err(|e| e.to_string())
}

fn as_signed_owned(pod: PodInfo) -> Option<SerializedSignedPod> {
    match pod.data {
        PodData::Signed(p) => Some(*p),
        PodData::Main(_) => None,
    }
}

fn description_for<'a>(frog: &'_ SignedPod, descs: &'a [SignedPod]) -> Option<&'a SignedPod> {
    let biome = frog.get("biome")?;
    let id = frog.id().0 .0[0].0;
    for desc in descs {
        if desc.get("biome") == Some(biome) {
            let RawValue([lo, hi, _, _]) = desc.get("seed_range")?.raw();
            if id >= lo.0 && id <= hi.0 {
                return Some(desc);
            }
        }
    }
    None
}

async fn frog_pods(db: &Db) -> Result<Vec<SignedPod>, String> {
    let mut infos = store::list_pods(db, "frogs")
        .await
        .map_err(|e| e.to_string())?;
    infos.sort_by(|i1, i2| i2.created_at.cmp(&i1.created_at));
    Ok(infos
        .into_iter()
        .filter_map(|info| SignedPod::try_from(as_signed_owned(info)?).ok())
        .collect())
}

async fn description_pods(db: &Db) -> Result<Vec<SignedPod>, String> {
    store::list_pods(db, "frog-descriptions")
        .await
        .map(|infos| {
            infos
                .into_iter()
                .filter_map(|info| SignedPod::try_from(as_signed_owned(info)?).ok())
                .collect()
        })
        .map_err(|e| e.to_string())
}

async fn request_frog_description(pod: SignedPod, app_handle: AppHandle) -> Result<(), String> {
    let client = Client::new();
    let url = server_url("desc");
    let desc: SignedPod = client
        .post(&url)
        .json(&pod)
        .send()
        .await
        .map_err(connection_failed)?
        .json()
        .await
        .map_err(connection_failed)?;
    let state = app_handle.state::<Mutex<AppState>>();
    let mut app_state = state.lock().await;
    register_pod(&mut app_state, desc, "frog-descriptions").await
}

async fn register_frog(app_state: &mut AppState, pod: SignedPod) -> Result<(), String> {
    let frog_descriptions = description_pods(&app_state.db).await?;
    if description_for(&pod, &frog_descriptions).is_none() {
        tauri::async_runtime::spawn(request_frog_description(
            pod.clone(),
            app_state.app_handle.clone(),
        ));
    }
    register_pod(app_state, pod, "frogs").await?;
    Ok(())
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
    /*
    if !space_exists(&app_state.db, "frogs")
        .await
        .map_err(|e| e.to_string())?
    {
        create_space(&app_state.db, "frogs")
            .await
            .map_err(|e| e.to_string())?;
    }
    */
    /*
    let name = match frog_response
        .pod
        .get("name")
        .map(pod2::middleware::Value::typed)
    {
        Some(TypedValue::String(s)) => Some(s.clone()),
        _ => None,
    };
    */
    /*
    store::import_pod(
        &app_state.db,
        &PodData::Signed(Box::new(frog_response.pod.into())),
        None,
        //name.as_deref(),
        "frogs",
    )
    .await
    .map_err(|e| format!("Failed to save POD: {e}"))?;
    app_state.trigger_state_sync().await?;
    */
    register_frog(&mut app_state, frog_response.pod).await?;
    Ok(frog_response.score)
}

#[tauri::command]
pub async fn fix_frog_descriptions(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    println!("trying to fix descriptions");
    let app_state = state.lock().await;
    let frog_pods = frog_pods(&app_state.db).await?;
    let frog_descs = description_pods(&app_state.db).await?;
    for pod in frog_pods {
        let desc = description_for(&pod, &frog_descs);
        if desc.is_none() {
            //request_frog_description(pod, app_state.app_handle.clone()).await?;
            tauri::async_runtime::spawn(request_frog_description(
                pod.clone(),
                app_state.app_handle.clone(),
            ));
        }
    }
    Ok(())
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

#[derive(Clone, Eq, PartialEq, Debug)]
struct WorkerData {
    biome: i64,
    private_key: SecretKey,
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
    biome: i64,
    max_depth: usize,
    nonce_key: RawValue,
}

const MAX_TRIES_BEFORE_POLLING: u64 = 50000;

const MINING_ZEROS_NEEDED: u64 = 27;
const MINING_ZERO_MASK: u64 = !((1 << (64 - MINING_ZEROS_NEEDED)) - 1);

impl FrogSearch {
    pub fn new(data: &WorkerData) -> Self {
        let mut kvs = std::collections::HashMap::new();
        let type_key: RawValue = hash_str(KEY_TYPE).into();
        let type_value: RawValue = (PodType::Signed as i64).into();
        let signer_key: RawValue = hash_str(KEY_SIGNER).into();
        let signer_value: RawValue = (&TypedValue::PublicKey(data.private_key.public_key())).into();
        let biome_key: RawValue = hash_str("biome").into();
        let biome_value: RawValue = data.biome.into();
        let nonce_key: RawValue = hash_str("nonce").into();
        let nonce_i64: i64 = OsRng.gen();
        let nonce_value: RawValue = nonce_i64.into();
        kvs.insert(type_key, type_value);
        kvs.insert(signer_key, signer_value);
        kvs.insert(biome_key, biome_value);
        kvs.insert(nonce_key, nonce_value);
        let max_depth = Params::default().max_depth_mt_containers;
        FrogSearch {
            kvs,
            max_depth,
            nonce_key,
            biome: data.biome,
        }
    }

    fn pod_hash(&self) -> Hash {
        MerkleTree::new(self.max_depth, &self.kvs).unwrap().root()
    }

    fn search_one(&mut self) -> bool {
        self.kvs.get_mut(&self.nonce_key).unwrap().0[3].0 += 1;
        let root = self.pod_hash();
        root.0[0].0 & MINING_ZERO_MASK == 0
    }

    pub fn search(&mut self) -> bool {
        for _ in 0..MAX_TRIES_BEFORE_POLLING {
            if self.search_one() {
                return true;
            }
        }
        false
    }

    fn generate_pod(&self, private_key: SecretKey) -> Result<SignedPod, String> {
        let mut builder = SignedPodBuilder::new(&Default::default());
        builder.insert("biome", self.biome);
        builder.insert("nonce", *self.kvs.get(&self.nonce_key).unwrap());
        let signer = Signer(private_key);
        builder.sign(&signer).map_err(|e| e.to_string())
    }
}

pub(crate) fn setup_background_thread(app_handle: AppHandle) {
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
                private_key: sk,
            });
            let mut worker_data_shared = sync_ui_clone.state.lock().unwrap();
            let enabled = worker_data.is_some();
            *worker_data_shared = worker_data;
            if enabled {
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
            let worker_data = worker_data_shared.clone();
            drop(worker_data_shared);
            if worker_data != old_worker_data {
                old_worker_data = worker_data.clone();
                search = worker_data.as_ref().map(FrogSearch::new);
            }
            let search_ref = search.as_mut().unwrap();
            if search_ref.search() {
                match search_ref.generate_pod(worker_data.unwrap().private_key) {
                    Ok(pod) => {
                        let app_handle_clone = app_handle.clone();
                        tauri::async_runtime::spawn(async move {
                            let state = app_handle_clone.state::<Mutex<AppState>>();
                            let mut app_state = state.lock().await;
                            register_frog(&mut app_state, pod)
                                .await
                                .map_err(|e| e.to_string())?;
                            app_handle_clone
                                .emit("frog-alert", "Found something in the mines!".to_string())
                                .map_err(|e| e.to_string())
                        });
                    }
                    Err(e) => log::error!("{e}"),
                }
            } else {
                count += 50;
                if let Err(e) = app_handle.emit("frog-background", count) {
                    log::error!("{e}");
                }
            }
        }
    });
}

#[derive(Serialize)]
pub struct FrogStats {
    jump: u64,
    speed: u64,
    intelligence: u64,
    beauty: u64,
    temperament: u64,
}

const DEFAULT_TEMPERAMENTS: [u64; 7] = [2, 3, 4, 7, 10, 16, 18];

fn compute_frog_stats(frog_id: i64, id: Hash) -> FrogStats {
    let val = id.0[1].0;
    match frog_id {
        81..=90 => FrogStats {
            jump: 0,
            speed: 0,
            intelligence: 0,
            beauty: val.rem_euclid(8) + 8,
            temperament: 1,
        },
        _ => {
            let (val, temperament_index) = val.div_rem_euclid(&7);
            let temperament = DEFAULT_TEMPERAMENTS[temperament_index as usize];
            let (val, beauty) = val.div_rem_euclid(&8);
            let (val, intelligence) = val.div_rem_euclid(&8);
            let (val, speed) = val.div_rem_euclid(&8);
            let jump = val.rem_euclid(8);
            FrogStats {
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
    use std::path::PathBuf;

    use pod2::{
        backends::plonky2::{primitives::ec::schnorr::SecretKey, signedpod::Signer},
        frontend::SignedPodBuilder,
        middleware::{hash_str, Params, PodId, RawValue},
    };
    use reqwest::Client;

    use crate::{
        config::AppConfig,
        frog::{download_frog, FrogSearch, WorkerData},
    };

    #[test]
    fn test_pod_hash() {
        let sk = SecretKey::new_rand();
        let biome = 1;
        let search = FrogSearch::new(&WorkerData {
            private_key: sk.clone(),
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
        let config_file = std::env::var("POD2_CONFIG_FILE").ok().map(PathBuf::from);
        let config = AppConfig::load_from_file(config_file).unwrap();
        AppConfig::initialize(config);
        let client = Client::new();
        let private_key = SecretKey::new_rand();
        let response = download_frog(&client, private_key).await?;
        println!("{}", response.text().await.unwrap());
        Ok(())
    }
}
