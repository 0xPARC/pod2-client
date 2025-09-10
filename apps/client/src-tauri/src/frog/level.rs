use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};

use pod2::{
    backends::plonky2::mainpod::Prover,
    frontend::{Error, MainPod, MainPodBuilder, Operation, SignedDict},
    middleware::{
        CustomPredicateBatch, CustomPredicateRef, Params, Predicate, Statement, DEFAULT_VD_SET,
        SELF_ID_HASH,
    },
};
use pod2_db::store::{get_pod, PodData, PodInfo};
use tauri::{AppHandle, Emitter, Event, Listener, Manager};

use crate::{
    frog::{log_err, register_frog},
    AppState,
};

pub(super) const LEVEL_UP_1: i64 = 4;
pub(super) const LEVEL_UP_2: i64 = 5;

pub(crate) fn setup_background_thread(app_handle: AppHandle) {
    let generation = Arc::new(AtomicU32::new(0));
    let app_handle_clone = app_handle.clone();
    let level_up_pred = build_level_up_pred();
    app_handle.listen("set-level-up", move |event| {
        process_toggle(&app_handle_clone, &generation, &level_up_pred, &event);
    });
}

fn process_toggle(
    app_handle: &AppHandle,
    generation: &Arc<AtomicU32>,
    level_up_pred: &Arc<CustomPredicateBatch>,
    event: &Event,
) {
    log::info!("got event {event:?}");
    if !event.payload().is_empty() {
        let thread_generation = generation.load(Ordering::Acquire);
        if let Ok(pod_id) = serde_json::from_str(event.payload()) {
            tauri::async_runtime::spawn(spawn_level_thread(
                app_handle.clone(),
                generation.clone(),
                thread_generation,
                level_up_pred.clone(),
                pod_id,
            ));
        }
    } else {
        generation.fetch_add(1, Ordering::Release);
    }
}

async fn spawn_level_thread(
    app_handle: AppHandle,
    generation: Arc<AtomicU32>,
    thread_generation: u32,
    level_up_pred: Arc<CustomPredicateBatch>,
    pod_id: String,
) {
    let state = app_handle.state::<tokio::sync::Mutex<AppState>>();
    let app_state = state.lock().await;
    if let Ok(Some(pod)) = get_pod(&app_state.db, "frogs", &pod_id).await {
        drop(app_state);
        log::info!("ready to spawn thread");
        std::thread::spawn(move || {
            level_up(
                app_handle,
                generation,
                thread_generation,
                level_up_pred,
                pod,
            );
        });
    }
}

struct LevelData {
    level: i64,
    goal: i64,
    level_statement: Statement,
    pod: MainPod,
    level_up_pred: Arc<CustomPredicateBatch>,
}

/*
impl LevelData {
    fn try_new(pod: PodInfo, level_up_pred: Arc<CustomPredicateBatch>) -> Option<Self> {
        match pod.data {
            PodData::Signed(s) => {
                Self::new_from_signed(&(*s).try_into().inspect_err(log_err).ok()?, level_up_pred)
                    .inspect_err(log_err)
                    .ok()
            }
            PodData::Main(m) => {
                Self::new_from_main((*m).try_into().inspect_err(log_err).ok()?, level_up_pred)
                    .inspect_err(log_err)
                    .ok()
            }
        }
    }

    fn level_up_helper(
        builder: &mut MainPodBuilder,
        level_up_pred: &Arc<CustomPredicateBatch>,
        prev_level: i64,
        prev_level_st: Statement,
    ) -> Result<Statement, Error> {
        let level = prev_level + 1;
        let sum_st = builder.priv_op(Operation::sum_of(level, prev_level, 1))?;
        let level_st = builder.priv_op(Operation::new_entry("level", level))?;
        let self_eq_st = builder.priv_op(Operation::eq(SELF_ID_HASH, SELF_ID_HASH))?;
        let level_up_rec_ref = CustomPredicateRef {
            batch: level_up_pred.clone(),
            index: 2,
        };
        let level_up_ref = CustomPredicateRef {
            batch: level_up_pred.clone(),
            index: 0,
        };
        let level_up_rec_st = builder.priv_op(Operation::custom(
            level_up_rec_ref,
            [prev_level_st, sum_st, level_st, self_eq_st],
        ))?;
        builder.pub_op(Operation::custom(
            level_up_ref,
            vec![Statement::None.into(), level_up_rec_st.into()],
        ))
    }

    fn new_from_signed(
        signed_pod: &SignedDict,
        level_up_pred: Arc<CustomPredicateBatch>,
    ) -> Result<Self, anyhow::Error> {
        let params = Default::default();
        let mut builder = MainPodBuilder::new(&params, &DEFAULT_VD_SET);
        builder.add_signed_pod(signed_pod);
        let level_up_base_ref = CustomPredicateRef {
            batch: level_up_pred.clone(),
            index: 1,
        };
        let level_up_ref = CustomPredicateRef {
            batch: level_up_pred.clone(),
            index: 0,
        };
        let lev_const_st = builder.priv_op(Operation::eq(1, 1))?;
        let biome_st = signed_pod
            .get_statement("biome")
            .ok_or_else(|| anyhow::anyhow!("missing biome"))?;
        builder.pub_op(Operation::copy(biome_st.clone()))?;
        let level_up_base_st = builder.priv_op(Operation::custom(
            level_up_base_ref,
            vec![lev_const_st.into(), biome_st.into()],
        ))?;
        let level_one_st = builder.priv_op(Operation::custom(
            level_up_ref.clone(),
            [level_up_base_st, Statement::None],
        ))?;
        let level_two_st = Self::level_up_helper(&mut builder, &level_up_pred, 1, level_one_st)?;
        let pod = builder.prove(&Prover {})?;
        Ok(Self {
            level: 2,
            goal: LEVEL_UP_1,
            pod,
            level_statement: level_two_st,
            level_up_pred,
        })
    }

    fn new_from_main(
        main_pod: MainPod,
        level_up_pred: Arc<CustomPredicateBatch>,
    ) -> Result<Self, anyhow::Error> {
        let params = Default::default();
        let level_up_ref = CustomPredicateRef {
            batch: level_up_pred.clone(),
            index: 0,
        };
        let level_up_st = main_pod
            .public_statements
            .iter()
            .filter(|st| st.predicate() == Predicate::Custom(level_up_ref.clone()))
            .next()
            .ok_or_else(|| anyhow::anyhow!("failed to find level up statement"))?
            .clone();
        let mut builder = MainPodBuilder::new(&params, &DEFAULT_VD_SET);
        builder.add_recursive_pod(main_pod);
        todo!()
    }

    pub fn level_up(&mut self) -> Result<(), Error> {
        let params = Default::default();
        let mut builder = MainPodBuilder::new(&params, &DEFAULT_VD_SET);
        builder.add_recursive_pod(self.pod.clone());
        builder.priv_op(Operation::copy(self.level_statement.clone()))?;
        self.level_statement = Self::level_up_helper(
            &mut builder,
            &self.level_up_pred,
            self.level,
            self.level_statement.clone(),
        )?;
        self.pod = builder.prove(&Prover {})?;
        self.level += 1;
        Ok(())
    }

    fn step(&mut self, app_handle: &AppHandle) -> bool {
        if self.level_up().is_err() {
            return false;
        }
        let finished = self.level >= self.goal;
        if finished {
            let pod = self.pod.clone();
            let app_handle_clone = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                let state = app_handle_clone.state::<tokio::sync::Mutex<AppState>>();
                let pod_id = pod.id().to_string();
                register_frog(&state, pod)
                    .await
                    .map_err(|e| e.to_string())?;
                app_handle_clone
                    .emit("frog-alert", "Finished leveling up!".to_string())
                    .map_err(|e| e.to_string())?;
                app_handle_clone
                    .emit("level-up-finish", pod_id)
                    .map_err(|e| e.to_string())
            });
        }
        !finished
    }
}
*/

fn level_up(
    app_handle: AppHandle,
    generation: Arc<AtomicU32>,
    thread_generation: u32,
    level_up_pred: Arc<CustomPredicateBatch>,
    pod: PodInfo,
) {
    /*
    if let Some(mut data) = LevelData::try_new(pod, level_up_pred) {
        while generation.load(Ordering::Acquire) == thread_generation && data.step(&app_handle) {
            if let Err(e) =
                app_handle.emit("level-up-status", format!("{}/{}", data.level, data.goal))
            {
                log_err(&e);
            }
        }
    }*/
}

fn build_level_up_pred() -> Arc<CustomPredicateBatch> {
    let st = format!(
        r#"
        level_up(origin_pod, level, private: proof_pod, shorter_level) = OR(
            level_up_base(?origin_pod, ?level)
            level_up_rec(?origin_pod, ?level, ?proof_pod, ?shorter_level)
        )

        level_up_base(origin_pod, level) = AND(
            Equal(?level, 1)
            Equal(?origin_pod["biome"], 1)
        )

        level_up_rec(origin_pod, level, proof_pod, shorter_level) = AND(
            level_up(?origin_pod, ?shorter_level)
            SumOf(?level, ?shorter_level, 1)
            Equal(?proof_pod["level"], ?level)
            Equal(?proof_pod, Raw({SELF_ID_HASH:#}))
        )
    "#,
    );
    pod2::lang::parse(&st, &Params::default(), &[])
        .unwrap()
        .custom_batch
}
