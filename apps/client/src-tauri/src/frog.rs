mod level;
mod mining;

pub(crate) use mining::setup_background_thread;
use num::traits::Euclid;
use pod2::{
    backends::plonky2::{primitives::ec::schnorr::SecretKey, signedpod::Signer},
    frontend::{MainPod, SerializedSignedPod, SignedPod, SignedPodBuilder},
    middleware::{Hash, PodId, RawValue, Statement, TypedValue, Value},
};
use pod2_db::{
    store::{self, create_space, space_exists, PodData, PodInfo},
    Db,
};
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
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
    rarity: i64,
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
    seen: bool,
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

fn frog_data_for(pod: &FrogPodInfo, desc: &SignedPod) -> Option<FrogData> {
    let frog_id = desc.get("frog_id")?.as_int()?;
    let rarity = if (1..=80).contains(&frog_id) {
        FROG_RARITIES[(frog_id - 1) as usize]
    } else {
        1
    };
    let name = desc.get("name")?.as_str()?.to_owned();
    let description = desc.get("description")?.as_str()?.to_owned();
    let image_url = desc.get("image_url")?.as_str()?.to_owned();
    let desc = FrogDesc {
        frog_id,
        name,
        description,
        image_url,
        rarity,
    };
    let stats = compute_frog_stats(frog_id, pod.base_id);
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
                pod_id: PodId(Hash(pod.pod_id.0)),
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
            seen: false,
        })
        .collect();
    for desc in frog_descs {
        if let Some((frog_id, name, image_url)) = frogedex_data_for(&desc) {
            if (1..=80).contains(&frog_id) {
                let index = (frog_id - 1) as usize;
                entries[index].name = name;
                entries[index].image_url = image_url;
                entries[index].seen = true;
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

fn description_for<'a>(frog: &'_ FrogPodInfo, descs: &'a [SignedPod]) -> Option<&'a SignedPod> {
    if (0..=1).contains(&frog.biome) {
        let offset = 2 * (frog.biome as usize);
        let id = frog.base_id.0[0].0;
        for desc in descs {
            let RawValue(arr) = desc.get("seed_range")?.raw();
            if id >= arr[offset].0 && id <= arr[offset + 1].0 {
                return Some(desc);
            }
        }
    }
    None
}

struct FrogPodInfo {
    pod_id: RawValue,
    base_id: RawValue,
    biome: i64,
    level: i64,
    json: Box<dyn FnOnce() -> String + Send + Sync>,
}

fn signed_pod_frog_info(pod: SignedPod) -> Option<FrogPodInfo> {
    let biome: i64 = pod.get("biome")?.typed().try_into().ok()?;
    let pod_id = RawValue::from(pod.id().0);
    let json = Box::new(move || serde_json::to_string(&pod).unwrap());
    Some(FrogPodInfo {
        pod_id,
        base_id: pod_id,
        biome,
        level: 0,
        json,
    })
}

fn get_frog_pod_info(pod: PodInfo) -> Option<FrogPodInfo> {
    match pod.data {
        PodData::Signed(s) => {
            let inner = SignedPod::try_from(s.as_ref().clone()).ok()?;
            signed_pod_frog_info(inner)
        }
        PodData::Main(s) => {
            let inner = MainPod::try_from(s.as_ref().clone()).ok()?;
            let statements = inner.pod.pub_statements();
            let (base_id, level) = statements
                .into_iter()
                .filter_map(|st| match st {
                    Statement::Custom(cpr, args) if cpr.index == 0 => {
                        let base_id = args.first()?.raw();
                        let level: i64 = args.get(1)?.typed().try_into().ok()?;
                        Some((base_id, level))
                    }
                    _ => None,
                })
                .next()?;
            let pod_id = RawValue::from(inner.id().0);
            let json = Box::new(move || serde_json::to_string(&inner).unwrap());
            Some(FrogPodInfo {
                pod_id,
                base_id,
                biome: 1,
                level,
                json,
            })
        }
    }
}

async fn frog_pods(db: &Db) -> Result<Vec<FrogPodInfo>, String> {
    let mut infos = store::list_pods(db, "frogs")
        .await
        .map_err(|e| e.to_string())?;
    infos.sort_by(|i1, i2| i2.created_at.cmp(&i1.created_at));
    Ok(infos.into_iter().filter_map(get_frog_pod_info).collect())
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

async fn request_frog_description(pod: String, app_handle: AppHandle) -> Result<(), String> {
    let client = Client::new();
    let url = server_url("desc");
    let desc: SignedPod = client
        .post(&url)
        .body(pod)
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
    let info = signed_pod_frog_info(pod.clone()).unwrap();
    if description_for(&info, &frog_descriptions).is_none() {
        tauri::async_runtime::spawn(request_frog_description(
            (info.json)(),
            app_state.app_handle.clone(),
        ));
    }
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
                (pod.json)(),
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

#[derive(Serialize)]
pub struct FrogStats {
    jump: u64,
    speed: u64,
    intelligence: u64,
    beauty: u64,
    temperament: u64,
}

const DEFAULT_TEMPERAMENTS: [u64; 7] = [2, 3, 4, 7, 10, 16, 18];

fn compute_frog_stats(frog_id: i64, id: RawValue) -> FrogStats {
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
            let bonus = if (61..=70).contains(&frog_id) { 8 } else { 0 };
            let (val, temperament_index) = val.div_rem_euclid(&7);
            let temperament = DEFAULT_TEMPERAMENTS[temperament_index as usize];
            let (val, beauty) = val.div_rem_euclid(&8);
            let (val, intelligence) = val.div_rem_euclid(&8);
            let (val, speed) = val.div_rem_euclid(&8);
            let jump = val.rem_euclid(8);
            FrogStats {
                jump: jump + bonus,
                speed: speed + bonus,
                intelligence: intelligence + bonus,
                beauty: beauty + bonus,
                temperament,
            }
        }
    }
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
