use std::sync::{Arc, Condvar};

use pod2::{
    backends::plonky2::{
        primitives::{ec::schnorr::SecretKey, merkletree::MerkleTree},
        signedpod::Signer,
    },
    frontend::{SignedPod, SignedPodBuilder},
    middleware::{hash_str, Hash, Params, PodType, RawValue, TypedValue, KEY_SIGNER, KEY_TYPE},
};
use pod2_db::store::get_default_private_key;
use rand::{rngs::OsRng, Rng};
use tauri::{AppHandle, Emitter, Listener, Manager};

use crate::{frog::register_frog, AppState};

const MAX_TRIES_BEFORE_POLLING: u64 = 50000;

const MINING_ZEROS_NEEDED: u64 = 29;
const MINING_ZERO_MASK: u64 = !((1 << (64 - MINING_ZEROS_NEEDED)) - 1);

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
                let state = app_handle_clone2.state::<tokio::sync::Mutex<AppState>>();
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
                            let state = app_handle_clone.state::<tokio::sync::Mutex<AppState>>();
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

#[cfg(test)]
mod test {
    use pod2::{
        backends::plonky2::{primitives::ec::schnorr::SecretKey, signedpod::Signer},
        frontend::SignedPodBuilder,
        middleware::{hash_str, Params, PodId, RawValue},
    };

    use crate::frog::mining::{FrogSearch, WorkerData};

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
}
