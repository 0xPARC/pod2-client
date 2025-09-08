use std::{collections::HashMap, time::Instant};

use pod2::{
    backends::plonky2::{mainpod::Prover, mock::mainpod::MockProver, signer::Signer},
    examples::MOCK_VD_SET,
    frontend::{MainPod, SignedDict, SignedDictBuilder},
    lang::{self, parser, LangError},
    middleware::{MainPodProver, Params, Value as PodValue, DEFAULT_VD_SET},
};
use pod2_db::{store, store::PodData};
use pod2_new_solver::{
    build_pod_from_answer_top_level_public, edb::ImmutableEdbBuilder, engine::Engine,
    EngineConfigBuilder, OpRegistry,
};
use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::Mutex;

use crate::AppState;

// =============================================================================
// Editor Types
// =============================================================================

/// Diagnostic severity levels for code validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

/// A diagnostic message from code validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub message: String,
    pub severity: DiagnosticSeverity,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

/// Response from code validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateCodeResponse {
    pub diagnostics: Vec<Diagnostic>,
}

/// Response from code execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteCodeResponse {
    pub main_pod: MainPod,
    pub diagram: String,
    pub solver_time_ms: u64,
    pub pod_build_time_ms: u64,
}

/// Convert LangError to diagnostics
fn lang_error_to_diagnostics(lang_error: &LangError) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let (message, start_line, start_col, end_line, end_col) = match lang_error {
        LangError::Parse(parse_error_box) => {
            let parser::ParseError::Pest(pest_error) = &**parse_error_box;
            let (sl, sc, el, ec) = match pest_error.line_col {
                pest::error::LineColLocation::Pos((l, c)) => (l, c, l, c),
                pest::error::LineColLocation::Span((sl, sc), (el, ec)) => (sl, sc, el, ec),
            };
            (format!("{}", pest_error.variant.message()), sl, sc, el, ec)
        }
        LangError::Processor(processor_error_box) => {
            let processor_error = &**processor_error_box;
            (format!("{processor_error}"), 1, 1, 1, 1)
        }
        LangError::Middleware(middleware_error_box) => {
            let middleware_error = &**middleware_error_box;
            (format!("{middleware_error}"), 1, 1, 1, 1)
        }
        LangError::Frontend(frontend_error_box) => {
            let frontend_error = &**frontend_error_box;
            (format!("{frontend_error}"), 1, 1, 1, 1)
        }
    };

    diagnostics.push(Diagnostic {
        message,
        severity: DiagnosticSeverity::Error,
        start_line: start_line as u32,
        start_column: start_col as u32,
        end_line: end_line as u32,
        end_column: end_col as u32,
    });

    diagnostics
}

/// Get information about the default private key
#[tauri::command]
pub async fn get_private_key_info(
    state: State<'_, Mutex<AppState>>,
) -> Result<serde_json::Value, String> {
    let app_state = state.lock().await;

    store::get_default_private_key_info(&app_state.db)
        .await
        .map_err(|e| format!("Failed to get private key info: {e}"))
}

/// Sign a POD with the given key-value pairs
#[tauri::command]
pub async fn sign_pod(
    state: State<'_, Mutex<AppState>>,
    serialized_pod_values: String,
) -> Result<String, String> {
    let app_state = state.lock().await;

    let kvs: HashMap<String, PodValue> = serde_json::from_str(&serialized_pod_values)
        .map_err(|e| format!("Failed to parse serialized pod values: {e}"))?;

    let params = Params::default();
    let mut builder = SignedDictBuilder::new(&params);
    for (key, value) in kvs {
        builder.insert(key, value);
    }

    // Get default private key (auto-created if needed)
    let private_key = store::get_default_private_key(&app_state.db)
        .await
        .map_err(|e| format!("Failed to get private key: {e}"))?;

    let signer = Signer(private_key);

    let signed_pod = builder
        .sign(&signer)
        .map_err(|e| format!("Failed to sign pod: {e}"))?;

    Ok(serde_json::to_string(&signed_pod).unwrap())
}

// =============================================================================
// Editor Commands
// =============================================================================

/// Validate Podlang code for syntax and semantic errors
#[tauri::command]
pub async fn validate_code_command(code: String) -> Result<ValidateCodeResponse, String> {
    log::debug!(
        "Validating code: {:?}",
        code.chars().take(50).collect::<String>()
    );

    let params = Params::default();
    pest::set_error_detail(true);

    match lang::parse(&code, &params, &[]) {
        Ok(_) => Ok(ValidateCodeResponse {
            diagnostics: vec![],
        }),
        Err(lang_error) => {
            log::debug!("Validation error: {lang_error:?}");
            let diagnostics = lang_error_to_diagnostics(&lang_error);
            Ok(ValidateCodeResponse { diagnostics })
        }
    }
}

/// Execute Podlang code against all available PODs
#[tauri::command]
pub async fn execute_code_command(
    state: State<'_, Mutex<AppState>>,
    code: String,
    mock: bool,
) -> Result<ExecuteCodeResponse, String> {
    log::debug!(
        "Executing code (mock: {}): {:?}",
        mock,
        code.chars().take(50).collect::<String>()
    );

    let app_state = state.lock().await;

    pest::set_error_detail(true);
    let params = Params::default();

    // Parse the code first
    let processed_output = match lang::parse(&code, &params, &[]) {
        Ok(output) => output,
        Err(e) => {
            log::error!("Failed to parse Podlang code: {e:?}");
            return Err(format!("Parse error: {e}"));
        }
    };

    if processed_output.request.templates().is_empty() {
        return Err("Program does not contain a POD Request".to_string());
    }

    // Get all PODs from all spaces
    let all_pod_infos = store::list_all_pods(&app_state.db)
        .await
        .map_err(|e| format!("Failed to list PODs: {e}"))?;

    if all_pod_infos.is_empty() {
        log::warn!("No PODs found for execution. Proceeding with empty facts.");
    }

    // Start solver timing
    let solver_start = Instant::now();

    let mut owned_signed_pods: Vec<SignedDict> = Vec::new();
    let mut owned_main_pods: Vec<MainPod> = Vec::new();

    // Convert stored PODs to runtime PODs
    for pod_info in all_pod_infos {
        // Sanity check: Ensure the pod_type string from DB matches the PodData enum variant type
        if pod_info.pod_type != pod_info.data.type_str() {
            log::warn!(
                "Data inconsistency for pod_id '{}' in space '{}' during execution: DB pod_type is '{}' but deserialized PodData is for '{}'. Trusting PodData enum.",
                pod_info.id, pod_info.space, pod_info.pod_type, pod_info.data.type_str()
            );
        }

        match pod_info.data {
            PodData::Signed(helper) => {
                owned_signed_pods.push(SignedDict::from(*helper));
            }
            PodData::Main(helper) => match MainPod::try_from(*helper) {
                Ok(main_pod) => {
                    owned_main_pods.push(main_pod);
                }
                Err(e) => {
                    log::error!(
                        "Failed to convert MainPodHelper to MainPod (id: {}, space: {}): {:?}",
                        pod_info.id,
                        pod_info.space,
                        e
                    );
                    return Err(format!(
                        "Failed to process stored pod data for pod id {} in space {}: {:?}",
                        pod_info.id, pod_info.space, e
                    ));
                }
            },
        }
    }

    let mut edb_builder = ImmutableEdbBuilder::new();
    for signed_dict in &owned_signed_pods {
        edb_builder = edb_builder.add_signed_dict(signed_dict.clone());
    }
    for main_pod in &owned_main_pods {
        edb_builder = edb_builder.add_main_pod(main_pod);
    }

    // let mut all_pods_for_facts: Vec<IndexablePod> = Vec::new();

    // for signed_pod_ref in &owned_signed_pods {
    //     // // If not in mock mode, Signed PODs must be of type Signed.
    //     // if !mock && signed_pod_ref.pod.pod_type().0 != PodType::Signed as usize {
    //     //     continue;
    //     // }
    //     all_pods_for_facts.push(IndexablePod::signed_pod(signed_pod_ref));
    // }

    // for main_pod_ref in &owned_main_pods {
    //     // If not in mock mode, Main PODs must be of type Main.
    //     if !mock && main_pod_ref.pod.pod_type().0 != PodType::Main as usize {
    //         continue;
    //     }
    //     all_pods_for_facts.push(IndexablePod::main_pod(main_pod_ref));
    // }

    let sk = store::get_default_private_key(&app_state.db)
        .await
        .map_err(|e| format!("Failed to get private key: {e}"))?
        .clone();
    // let sks = vec![sk];

    edb_builder = edb_builder.add_keypair(sk.public_key(), sk);
    let engine_config = EngineConfigBuilder::new().from_params(&params);
    let reg = OpRegistry::default();
    let edb = edb_builder.build();
    let mut engine = Engine::with_config(&reg, &edb, engine_config.build());

    engine.load_processed(&processed_output);
    engine.run().expect("run ok");

    // End solver timing
    let solver_time = solver_start.elapsed();

    // Choose VD set based on mock mode
    #[allow(clippy::borrow_interior_mutable_const)]
    let vd_set = if mock { &MOCK_VD_SET } else { &*DEFAULT_VD_SET };

    // Create prover based on mock mode
    let prover: Box<dyn MainPodProver> = if mock {
        Box::new(MockProver {})
    } else {
        Box::new(Prover {})
    };

    // Start POD build timing
    let pod_build_start = Instant::now();

    let pod = build_pod_from_answer_top_level_public(
        &engine.answers[0],
        &params,
        vd_set,
        |b| b.prove(&*prover).map_err(|e| e.to_string()),
        &edb,
    )
    .unwrap();

    // let mut builder = MainPodBuilder::new(&params, vd_set);
    // for (operation, public) in ops {
    //     if public {
    //         builder.pub_op(operation).unwrap();
    //     } else {
    //         builder.priv_op(operation).unwrap();
    //     }
    // }
    // for pod_id in pod_ids {
    //     let pod = all_pods_for_facts.iter().find(|p| p.id() == pod_id);
    //     if let Some(pod) = pod {
    //         match pod {
    //             IndexablePod::SignedPod(pod) => {
    //                 //    builder.add_signed_pod(pod);
    //             }
    //             IndexablePod::MainPod(pod) => {
    //                 builder.add_pod(pod.as_ref().clone());
    //             }
    //             IndexablePod::TestPod(_pod) => {}
    //         }
    //     }
    // }

    // let result_main_pod = builder
    //     .prove(&*prover)
    //     .map_err(|e| format!("Failed to prove: {e}"))?;

    // End POD build timing
    let pod_build_time = pod_build_start.elapsed();

    let result = ExecuteCodeResponse {
        main_pod: pod,
        diagram: "".to_string(),
        solver_time_ms: solver_time.as_millis() as u64,
        pod_build_time_ms: pod_build_time.as_millis() as u64,
    };

    Ok(result)
}
