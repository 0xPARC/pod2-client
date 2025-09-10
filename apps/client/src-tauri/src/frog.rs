mod level;
mod mining;
mod types;

use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
};

use num::traits::Euclid;
use pod2::{
    backends::plonky2::{
        primitives::ec::{curve::Point, schnorr::SecretKey},
        signer::Signer,
    },
    frontend::{MainPod, SignedDict, SignedDictBuilder},
    middleware::{Hash, Key, RawValue, Statement, TypedValue, Value},
};
use pod2_db::{
    store::{self, create_space, space_exists, PodData, PodInfo, SignedDictWrapper},
    Db,
};
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

use crate::{
    config::config,
    frog::types::{
        compute_frog_stats, description_for, get_frog_pod_info, AsTyped, Frog, FrogDerived,
        FrogPodInfo, FrogedexData, IntoFrogPod, SerializablePod,
    },
    AppState,
};

fn server_url(path: &str) -> String {
    let domain = &config().network.frogcrypto_server;
    format!("{domain}/{path}")
}

fn connection_failed<T>(_: T) -> String {
    "failed to connect to server".to_string()
}

fn log_err<E: Display + ?Sized>(e: &E) {
    log::error!("{e}");
}

#[derive(Deserialize)]
struct Challenge {
    public_key: String,
    time: String,
}

#[derive(Deserialize)]
struct FrogResponse {
    pod: SignedDict,
    score: i64,
}

pub(crate) fn setup_background_threads(app_handle: &AppHandle) {
    self::level::setup_background_thread(app_handle.clone());
    self::mining::setup_background_thread(app_handle.clone());
}

async fn process_challenge(client: &Client, private_key: SecretKey) -> Result<SignedDict, String> {
    let challenge_url = server_url("auth");
    let challenge: Challenge = client
        .get(&challenge_url)
        .send()
        .await
        .map_err(connection_failed)?
        .json()
        .await
        .map_err(connection_failed)?;
    let mut builder = SignedDictBuilder::new(&Default::default());
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

/*
impl FrogPod {
    fn index(&self) -> Option<usize> {
        let frog_id = self.data.as_ref()?.desc.frog_id;
        if (1..=80).contains(&frog_id) {
            Some((frog_id - 1) as usize)
        } else {
            None
        }
    }

    fn can_level_up(&self) -> bool {
        if let Some(data) = &self.data {
            data.desc.can_level_up
        } else {
            false
        }
    }
}
*/

const RARE_RARITY: i64 = 1;
const EPIC_RARITY: i64 = 2;
const JUNK_RARITY: i64 = 10;

const FROG_RARITIES: [i64; 201] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 1, 2,
    1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2,
    1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2,
    1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 6, 5, 5, 5, 5, 7,
    9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 10, 10, 10, 10, 10, 10, 10, 10, 10,
    10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 8,
];

fn index_for_id(frog_id: i64) -> Option<usize> {
    if (1..=FROG_RARITIES.len() as i64).contains(&frog_id) {
        Some((frog_id as usize) - 1)
    } else {
        None
    }
}

fn rarity_for(frog_id: i64) -> i64 {
    if let Some(index) = index_for_id(frog_id) {
        FROG_RARITIES[index]
    } else {
        0
    }
}

#[tauri::command]
pub async fn list_frogs(state: State<'_, Mutex<AppState>>) -> Result<Vec<Frog>, String> {
    let app_state = state.lock().await;
    let frog_pods = frog_pods(&app_state.db).await?;
    //let mut epics_seen = HashSet::new();
    let frog_descs = description_pods(&app_state.db).await?;
    drop(app_state);
    let frogs: Vec<_> = frog_pods
        .into_iter()
        .map(|pod| {
            let desc = description_for(&pod, &frog_descs);
            let derived = desc.map(|d| FrogDerived::from_info(&pod, d));
            let pod_id = pod.id;
            let level_up_indicator = if pod.leveled_up { "+" } else { "" };
            let id = format!("{pod_id:#}{level_up_indicator}");
            // TODO: allow rare to be leveled up if there is no corresponding
            // epic frog
            let frog_pod = Frog {
                id,
                derived,
                offer_level_up: false,
            };
            frog_pod
        })
        .collect();
    Ok(frogs)
}

#[tauri::command]
pub async fn get_frogedex(state: State<'_, Mutex<AppState>>) -> Result<Vec<FrogedexData>, String> {
    let app_state = state.lock().await;
    let frog_descs = description_pods(&app_state.db).await?;
    drop(app_state);
    let mut entries: Vec<_> = FROG_RARITIES
        .iter()
        .enumerate()
        .map(|(n, &rarity)| FrogedexData {
            frog_id: (n + 1) as i64,
            rarity,
            seed_range: Default::default(),
            name: "???".to_string(),
            image_url: "https://frogcrypto.vercel.app/images/pixel_frog.png".to_string(),
            description: Default::default(),
            seen: false,
        })
        .collect();
    for data in frog_descs {
        if let Some(index) = index_for_id(data.frog_id) {
            entries[index].name = data.name;
            entries[index].image_url = data.image_url;
            entries[index].seen = true;
        }
    }
    Ok(entries)
}

async fn register_pod(
    app_state: &mut AppState,
    pod: impl IntoFrogPod,
    space: &str,
) -> Result<(), String> {
    if !space_exists(&app_state.db, space)
        .await
        .map_err(|e| e.to_string())?
    {
        create_space(&app_state.db, space)
            .await
            .map_err(|e| e.to_string())?;
    }
    store::import_pod(&app_state.db, &pod.pod_data(), None, space)
        .await
        .map_err(|e| e.to_string())?;
    app_state.trigger_state_sync().await?;
    app_state
        .app_handle
        .emit("refresh-frogs", ())
        .map_err(|e| e.to_string())
}

fn as_signed_owned(pod: PodInfo) -> Option<SignedDictWrapper> {
    match pod.data {
        PodData::Signed(p) => Some(*p),
        PodData::Main(_) => None,
    }
}

#[derive(Serialize)]
enum TaggedPod {
    Signed(SignedDict),
    Main(MainPod),
}

#[derive(Serialize)]
struct FrogRegistry {
    public_key: Point,
    frogs: Vec<TaggedPod>,
}

async fn frog_pods(db: &Db) -> Result<Vec<FrogPodInfo>, String> {
    let mut infos = store::list_pods(db, "frogs")
        .await
        .map_err(|e| e.to_string())?;
    infos.sort_by(|i1, i2| i2.created_at.cmp(&i1.created_at));
    Ok(infos.into_iter().filter_map(get_frog_pod_info).collect())
}

async fn description_pods(db: &Db) -> Result<Vec<FrogedexData>, String> {
    store::list_pods(db, "frog-descriptions")
        .await
        .map(|infos| {
            infos
                .into_iter()
                .filter_map(|info| {
                    FrogedexData::from_pod(&SignedDict::try_from(as_signed_owned(info)?).ok()?)
                })
                .collect()
        })
        .map_err(|e| e.to_string())
}

async fn request_frog_description_and_log_err(pod: SerializablePod, app_handle: AppHandle) {
    if let Err(e) = request_frog_description(pod, app_handle).await {
        log::error!("{e}");
    }
}

async fn request_frog_description(
    pod: SerializablePod,
    app_handle: AppHandle,
) -> Result<(), String> {
    let client = Client::new();
    let route = match &pod {
        SerializablePod::Signed(_) => "desc",
        SerializablePod::Main(_) => "desc2",
    };
    let url = server_url(route);
    let desc: SignedDict = client
        .post(&url)
        .json(&pod)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let state = app_handle.state::<Mutex<AppState>>();
    let mut app_state = state.lock().await;
    register_pod(&mut app_state, desc, "frog-descriptions").await
}

async fn register_frog(
    state: &State<'_, Mutex<AppState>>,
    pod: impl IntoFrogPod,
) -> Result<(), String> {
    let mut app_state = state.lock().await;
    register_pod(&mut app_state, pod.clone(), "frogs").await?;
    let frog_descriptions = description_pods(&app_state.db).await?;
    let app_handle = app_state.app_handle.clone();
    drop(app_state);
    let info = pod
        .clone()
        .info()
        .ok_or_else(|| "failed to parse pod".to_string())?;
    if description_for(&info, &frog_descriptions).is_none() {
        tauri::async_runtime::spawn(request_frog_description_and_log_err(
            info.pod,
            app_handle.clone(),
        ));
    }
    Ok(())
}

#[tauri::command]
pub async fn request_frog(state: State<'_, Mutex<AppState>>) -> Result<i64, String> {
    let app_state = state.lock().await;
    let private_key = crate::get_private_key(&app_state.db).await?;
    drop(app_state);
    let client = Client::new();
    let frog_response: FrogResponse = download_frog(&client, private_key)
        .await?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    register_frog(&state, frog_response.pod).await?;
    Ok(frog_response.score)
}

#[tauri::command]
pub async fn fix_frog_descriptions(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let app_state = state.lock().await;
    let frog_pods = frog_pods(&app_state.db).await?;
    let frog_descs = description_pods(&app_state.db).await?;
    let app_handle = app_state.app_handle.clone();
    drop(app_state);
    for pod in frog_pods {
        let desc = description_for(&pod, &frog_descs);
        if desc.is_none() {
            tauri::async_runtime::spawn(request_frog_description_and_log_err(
                pod.pod,
                app_handle.clone(),
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
    drop(app_state);
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

fn filter_proof_of_work(pod: FrogPodInfo) -> Option<TaggedPod> {
    match pod.pod {
        SerializablePod::Signed(p) => {
            if p.get("biome").map(|v| v.typed()) == Some(&TypedValue::Int(1)) {
                Some(TaggedPod::Signed(p))
            } else {
                None
            }
        }
        // TODO
        SerializablePod::Main(_) => None,
    }
}

#[tauri::command]
pub async fn reregister_all_frogs(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let app_state = state.lock().await;
    let private_key = crate::get_private_key(&app_state.db).await?;
    let frog_pods = frog_pods(&app_state.db).await?;
    let app_handle = app_state.app_handle.clone();
    drop(app_state);
    let frogs_to_register = frog_pods
        .into_iter()
        .filter_map(filter_proof_of_work)
        .collect();
    upload_frogs(
        app_handle,
        FrogRegistry {
            public_key: private_key.public_key(),
            frogs: frogs_to_register,
        },
    )
    .await
}

async fn upload_frogs(app_handle: AppHandle, frogs: FrogRegistry) -> Result<(), String> {
    let client = Client::new();
    let url = server_url("register");
    let score: i64 = client
        .post(&url)
        .json(&frogs)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    app_handle
        .emit("update-score", score)
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

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use pod2::backends::plonky2::primitives::ec::schnorr::SecretKey;
    use reqwest::Client;

    use crate::{config::AppConfig, frog::download_frog};

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
