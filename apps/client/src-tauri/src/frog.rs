mod level;
mod mining;

use std::fmt::Display;

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

use crate::{config::config, frog::level::LEVEL_UP_2, AppState};

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
    pod: SignedPod,
    score: i64,
}

pub(crate) fn setup_background_threads(app_handle: &AppHandle) {
    self::level::setup_background_thread(app_handle.clone());
    self::mining::setup_background_thread(app_handle.clone());
}

async fn get_private_key(state: &State<'_, Mutex<AppState>>) -> Result<SecretKey, String> {
    let app_state = state.lock().await;
    crate::get_private_key(&app_state.db).await
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
pub struct FrogData {
    #[serde(flatten)]
    desc: FrogedexData,
    #[serde(flatten)]
    stats: FrogStats,
}

#[derive(Serialize)]
pub struct FrogPod {
    id: PodId,
    data: Option<FrogData>,
    level: i64,
    offer_level_up: bool,
}

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

#[derive(Serialize, Clone)]
pub struct FrogedexData {
    frog_id: i64,
    level: i64,
    rarity: i64,
    seed_range: RawValue,
    name: String,
    image_url: String,
    description: String,
    can_level_up: bool,
    seen: bool,
}

const FROG_RARITIES: [i64; 80] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 4, 3,
    3, 3, 4, 3, 3, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
];

trait AsTyped {
    fn as_str(&self) -> Option<&str>;
    fn as_string(&self) -> Option<String>;
    fn as_int(&self) -> Option<i64>;
    fn as_bool(&self) -> Option<bool>;
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

    fn as_bool(&self) -> Option<bool> {
        match self.typed() {
            TypedValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    fn as_string(&self) -> Option<String> {
        match self.typed() {
            TypedValue::String(s) => Some(s.clone()),
            _ => None,
        }
    }
}

fn rarity_for(frog_id: i64) -> i64 {
    if (1..=80).contains(&frog_id) {
        FROG_RARITIES[(frog_id - 1) as usize]
    } else {
        1
    }
}

fn frog_data_for(pod: &FrogPodInfo, desc: &FrogedexData) -> FrogData {
    let stats = compute_frog_stats(desc.frog_id, pod.base_id);
    FrogData {
        desc: desc.clone(),
        stats,
    }
}

#[tauri::command]
pub async fn list_frogs(state: State<'_, Mutex<AppState>>) -> Result<Vec<FrogPod>, String> {
    let app_state = state.lock().await;
    let frog_pods = frog_pods(&app_state.db).await?;
    let mut max_levels = [0; 80];
    let frog_descs = description_pods(&app_state.db).await?;
    drop(app_state);
    let mut frogs: Vec<_> = frog_pods
        .into_iter()
        .map(|pod| {
            let desc = description_for(&pod, &frog_descs);
            let data = desc.map(|d| frog_data_for(&pod, d));
            let frog_pod = FrogPod {
                id: PodId(Hash(pod.pod_id.0)),
                data,
                level: pod.level,
                offer_level_up: false,
            };
            if let Some(idx) = frog_pod.index() {
                if pod.level > max_levels[idx] {
                    max_levels[idx] = pod.level;
                }
            }

            frog_pod
        })
        .collect();
    for frog in frogs.iter_mut() {
        if let Some(idx) = frog.index() {
            if frog.can_level_up() && frog.level == max_levels[idx] && frog.level < LEVEL_UP_2 {
                frog.offer_level_up = true;
            }
        }
    }
    Ok(frogs)
}

fn frogedex_data_for(desc: &SignedPod) -> Option<FrogedexData> {
    // for backward compatibility, interpret missing level as 0
    let level = if let Some(lvl) = desc.get("level") {
        lvl.as_int()?
    } else {
        0
    };
    let frog_id = desc.get("frog_id")?.as_int()?;
    Some(FrogedexData {
        frog_id,
        level,
        rarity: rarity_for(frog_id),
        seed_range: desc.get("seed_range")?.raw(),
        name: desc.get("name")?.as_string()?,
        image_url: desc.get("image_url")?.as_string()?,
        description: desc.get("description")?.as_string()?,
        can_level_up: desc.get("can_level_up")?.as_bool()?,
        seen: true,
    })
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
            level: 0,
            rarity,
            seed_range: Default::default(),
            name: "???".to_string(),
            image_url: "https://frogcrypto.vercel.app/images/pixel_frog.png".to_string(),
            description: Default::default(),
            seen: false,
            can_level_up: false,
        })
        .collect();
    for data in frog_descs {
        if (1..=80).contains(&data.frog_id) && data.level == 0 {
            let index = (data.frog_id - 1) as usize;
            entries[index].name = data.name;
            entries[index].image_url = data.image_url;
            entries[index].seen = true;
            entries[index].can_level_up = data.can_level_up;
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

fn as_signed_owned(pod: PodInfo) -> Option<SerializedSignedPod> {
    match pod.data {
        PodData::Signed(p) => Some(*p),
        PodData::Main(_) => None,
    }
}

fn description_for<'a>(
    frog: &'_ FrogPodInfo,
    descs: &'a [FrogedexData],
) -> Option<&'a FrogedexData> {
    if (0..=1).contains(&frog.biome) {
        let offset = 2 * (frog.biome as usize);
        let id = frog.base_id.0[0].0;
        descs
            .iter()
            .filter(|desc| {
                frog.level == desc.level
                    && id >= desc.seed_range.0[offset].0
                    && id <= desc.seed_range.0[offset + 1].0
            })
            .next()
    } else {
        None
    }
}

trait IntoFrogPod: Clone {
    fn info(self) -> Option<FrogPodInfo>;
    fn pod_data(self) -> PodData;
}

struct FrogPodInfo {
    pod_id: RawValue,
    base_id: RawValue,
    biome: i64,
    level: i64,
    json: Box<dyn FnOnce() -> serde_json::Value + Send + Sync>,
}

impl IntoFrogPod for SignedPod {
    fn info(self) -> Option<FrogPodInfo> {
        let biome: i64 = self.get("biome")?.typed().try_into().ok()?;
        let pod_id = RawValue::from(self.id().0);
        let json = Box::new(move || serde_json::to_value(&self).unwrap());
        Some(FrogPodInfo {
            pod_id,
            base_id: pod_id,
            biome,
            level: 0,
            json,
        })
    }
    fn pod_data(self) -> PodData {
        PodData::Signed(Box::new(self.into()))
    }
}

impl IntoFrogPod for MainPod {
    fn info(self) -> Option<FrogPodInfo> {
        let statements = self.pod.pub_statements();
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
        let pod_id = RawValue::from(self.id().0);
        let json = Box::new(move || serde_json::to_value(&self).unwrap());
        Some(FrogPodInfo {
            pod_id,
            base_id,
            biome: 1,
            level,
            json,
        })
    }
    fn pod_data(self) -> PodData {
        PodData::Main(Box::new(self.into()))
    }
}

fn get_frog_pod_info(pod: PodInfo) -> Option<FrogPodInfo> {
    match pod.data {
        PodData::Signed(s) => {
            let inner = SignedPod::try_from(s.as_ref().clone()).ok()?;
            inner.info()
        }
        PodData::Main(s) => {
            let inner = MainPod::try_from(s.as_ref().clone()).ok()?;
            inner.info()
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

async fn description_pods(db: &Db) -> Result<Vec<FrogedexData>, String> {
    store::list_pods(db, "frog-descriptions")
        .await
        .map(|infos| {
            infos
                .into_iter()
                .filter_map(|info| {
                    frogedex_data_for(&SignedPod::try_from(as_signed_owned(info)?).ok()?)
                })
                .collect()
        })
        .map_err(|e| e.to_string())
}

async fn request_frog_description_and_log_err(pod: serde_json::Value, app_handle: AppHandle) {
    if let Err(e) = request_frog_description(pod, app_handle).await {
        log::error!("{e}");
    }
}

async fn request_frog_description(
    pod: serde_json::Value,
    app_handle: AppHandle,
) -> Result<(), String> {
    let client = Client::new();
    let url = server_url("desc");
    let desc: SignedPod = client
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
            (info.json)(),
            app_handle.clone(),
        ));
    }
    Ok(())
}

#[tauri::command]
pub async fn request_frog(state: State<'_, Mutex<AppState>>) -> Result<i64, String> {
    let private_key = get_private_key(&state).await?;
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
                (pod.json)(),
                app_handle.clone(),
            ));
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn request_score(state: State<'_, Mutex<AppState>>) -> Result<serde_json::Value, String> {
    let client = Client::new();
    let private_key = get_private_key(&state).await?;
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
