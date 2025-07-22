use std::collections::HashMap;

use pod2::{
    backends::plonky2::{mainpod::Prover, mock::mainpod::MockProver},
    examples::MOCK_VD_SET,
    frontend::{MainPodBuilder, SerializedMainPod},
    lang,
    middleware::{Params, DEFAULT_VD_SET},
};
use pod2_db::store;
use pod2_solver::{db::IndexablePod, metrics::MetricsLevel};
use tauri::State;
use tokio::sync::Mutex;

use crate::{get_feature_config, AppState};

/// Macro to check if integration feature is enabled
macro_rules! check_feature_enabled {
    () => {
        if !get_feature_config().integration {
            log::warn!("Integration feature is disabled");
            return Err("Integration feature is disabled".to_string());
        }
    };
}

/// Submit a POD request and get back a MainPod proof
#[tauri::command]
pub async fn submit_pod_request(
    state: State<'_, Mutex<AppState>>,
    request: String,
) -> Result<SerializedMainPod, String> {
    check_feature_enabled!();
    log::info!("request: {request}");
    let params = Params::default();
    let pod_request = lang::parse(request.as_str(), &params, &[]).unwrap();

    #[allow(unused_variables)]
    let mock = false;
    #[cfg(debug_assertions)]
    let mock = true;

    let mut app_state = state.lock().await;
    let fetched_pod_infos = store::list_all_pods(&app_state.db)
        .await
        .map_err(|e| format!("Failed to list pods: {e}"))?;

    let mut owned_signed_pods = Vec::new();
    let mut owned_main_pods = Vec::new();

    for pod_info in fetched_pod_infos {
        // Sanity check: Ensure the pod_type string from DB matches the PodData enum variant type
        if pod_info.pod_type != pod_info.data.type_str() {
            log::warn!(
                "Data inconsistency for pod_id '{}' in space '{}' during execution: DB pod_type is '{}' but deserialized PodData is for '{}'. Trusting PodData enum.",
                pod_info.id, crate::DEFAULT_SPACE_ID, pod_info.pod_type, pod_info.data.type_str()
            );
        }

        match pod_info.data {
            pod2_db::store::PodData::Signed(helper) => {
                owned_signed_pods.push(pod2::frontend::SignedPod::try_from(*helper).unwrap());
            }
            pod2_db::store::PodData::Main(helper) => {
                match pod2::frontend::MainPod::try_from(*helper) {
                    Ok(main_pod) => {
                        owned_main_pods.push(main_pod);
                    }
                    Err(e) => {
                        log::error!(
                            "Failed to convert MainPodHelper to MainPod (id: {}, space: {}): {:?}",
                            pod_info.id,
                            crate::DEFAULT_SPACE_ID,
                            e
                        );
                        return Err(format!(
                            "Failed to convert MainPodHelper to MainPod (id: {}, space: {}): {:?}",
                            pod_info.id,
                            crate::DEFAULT_SPACE_ID,
                            e
                        ));
                    }
                }
            }
            pod2_db::store::PodData::RsaIntro(_) => {
                // RSA intro PODs are not used in integration requests, skip them
                log::debug!("Skipping RSA intro POD {} during integration request", pod_info.id);
            }
        }
    }

    let mut all_pods_for_facts = Vec::new();
    let mut original_signed_pods = HashMap::new();
    let mut original_main_pods = HashMap::new();

    for signed_pod_ref in &owned_signed_pods {
        // If not in mock mode, Signed PODs must be of type Signed.
        if !mock && signed_pod_ref.pod.pod_type().0 != pod2::middleware::PodType::Signed as usize {
            continue;
        }
        all_pods_for_facts.push(IndexablePod::signed_pod(signed_pod_ref));
        original_signed_pods.insert(signed_pod_ref.id(), signed_pod_ref);
    }

    for main_pod_ref in &owned_main_pods {
        // If not in mock mode, Main PODs must be of type Main.
        if !mock && main_pod_ref.pod.pod_type().0 != pod2::middleware::PodType::Main as usize {
            continue;
        }
        all_pods_for_facts.push(IndexablePod::main_pod(main_pod_ref));
        original_main_pods.insert(main_pod_ref.id(), main_pod_ref);
    }

    let request_templates = pod_request.request_templates;

    let (proof, _) =
        match pod2_solver::solve(&request_templates, &all_pods_for_facts, MetricsLevel::None) {
            Ok(solution) => solution,
            Err(e) => {
                log::error!("Solver error: {e:?}");
                return Err(format!("Solver error: {e:?}, request: {request}"));
            }
        };

    let (pod_ids, ops) = proof.to_inputs();

    let vd_set = if mock {
        #[allow(clippy::borrow_interior_mutable_const)]
        &*MOCK_VD_SET
    } else {
        &*DEFAULT_VD_SET
    };

    let mut builder = MainPodBuilder::new(&params, vd_set);
    for (operation, public) in ops {
        if public {
            builder.pub_op(operation).unwrap();
        } else {
            builder.priv_op(operation).unwrap();
        }
    }
    for pod_id in pod_ids {
        let pod = all_pods_for_facts.iter().find(|p| p.id() == pod_id);
        if let Some(pod) = pod {
            match pod {
                IndexablePod::SignedPod(pod) => {
                    builder.add_signed_pod(pod);
                }
                IndexablePod::MainPod(pod) => {
                    builder.add_recursive_pod(pod.as_ref().clone());
                }
                IndexablePod::TestPod(_pod) => {}
            }
        }
    }
    let result_main_pod = if mock {
        let prover = MockProver {};
        builder.prove(&prover, &params).unwrap()
    } else {
        let prover = Prover {};
        builder.prove(&prover, &params).unwrap()
    };
    let serialized_pod: SerializedMainPod = result_main_pod.into();

    // Trigger state sync after creating the pod
    app_state.trigger_state_sync().await?;

    Ok(serialized_pod)
}
