use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};

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
use tauri::{AppHandle, Emitter, Event, Listener, Manager};

use crate::{frog::register_frog, AppState};

const MAX_TRIES_BEFORE_POLLING: u64 = 50000;

const MINING_ZEROS_NEEDED: u64 = 29;
const MINING_ZERO_MASK: u64 = !((1 << (64 - MINING_ZEROS_NEEDED)) - 1);

pub(crate) fn setup_background_thread(app_handle: AppHandle) {
    // We use `generation` as a signal to disable the mining thread.  When
    // we increment `generation`, the thread will stop.
    let generation = Arc::new(AtomicU32::new(0));
    let app_handle_clone = app_handle.clone();
    app_handle.listen("toggle-mining", move |event| {
        process_toggle(app_handle_clone.clone(), generation.clone(), event)
    });
}

fn process_toggle(app_handle: AppHandle, generation: Arc<AtomicU32>, event: Event) {
    if event.payload() == "true" {
        let thread_generation = generation.load(Ordering::Acquire);
        tauri::async_runtime::spawn(spawn_mining_thread(
            app_handle,
            generation,
            thread_generation,
        ));
    } else {
        generation.fetch_add(1, Ordering::Release);
    }
}

async fn spawn_mining_thread(
    app_handle: AppHandle,
    generation: Arc<AtomicU32>,
    thread_generation: u32,
) {
    let state = app_handle.state::<tokio::sync::Mutex<AppState>>();
    let app_state = state.lock().await;
    if let Ok(private_key) = get_default_private_key(&app_state.db).await {
        drop(app_state);
        std::thread::spawn(move || mine(app_handle, generation, thread_generation, private_key));
    }
}

fn mine(
    app_handle: AppHandle,
    generation: Arc<AtomicU32>,
    thread_generation: u32,
    private_key: SecretKey,
) {
    let mut worker = MiningData {
        private_key,
        biome: 1,
    }
    .make_worker();
    while generation.load(Ordering::Acquire) == thread_generation {
        worker.step(&app_handle);
    }
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub(super) struct MiningData {
    biome: i64,
    private_key: SecretKey,
}

impl MiningData {
    pub(crate) fn make_worker(&self) -> MiningWorker {
        MiningWorker {
            search: FrogSearch::new(self),
            private_key: self.private_key.clone(),
            count: 0,
        }
    }
}

pub(super) struct MiningWorker {
    search: FrogSearch,
    private_key: SecretKey,
    count: usize,
}

impl MiningWorker {
    fn step(&mut self, app_handle: &AppHandle) {
        if self.search.search() {
            match self.search.generate_pod(self.private_key.clone()) {
                Ok(pod) => {
                    let app_handle_clone = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        let state = app_handle_clone.state::<tokio::sync::Mutex<AppState>>();
                        register_frog(&state, pod)
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
            self.count += 50;
            if let Err(e) = app_handle.emit("frog-background", self.count) {
                log::error!("{e}");
            }
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
    pub fn new(data: &MiningData) -> Self {
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

#[cfg(test)]
mod test {
    use pod2::{
        backends::plonky2::{primitives::ec::schnorr::SecretKey, signedpod::Signer},
        frontend::SignedPodBuilder,
        middleware::{hash_str, Params, PodId, RawValue},
    };

    use crate::frog::mining::{FrogSearch, MiningData};

    #[test]
    fn test_pod_hash() {
        let sk = SecretKey::new_rand();
        let biome = 1;
        let search = FrogSearch::new(&MiningData {
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
