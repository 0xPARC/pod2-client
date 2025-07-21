use std::collections::HashMap;

use hex::ToHex;
use pod2::{
    backends::plonky2::{mainpod::Prover, mock::mainpod::MockProver, signedpod::Signer},
    examples::MOCK_VD_SET,
    frontend::{MainPod, MainPodBuilder, SignedPod, SignedPodBuilder},
    lang::{self, parser, LangError},
    middleware::{Params, PodProver, PodType, Value as PodValue, DEFAULT_VD_SET},
};
use pod2_db::{store, store::PodData};
use pod2_solver::{self, db::IndexablePod, metrics::MetricsLevel};
use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::Mutex;

use crate::{
    features::console::commands::{
        log_error_event_from_app_handle, log_pod_operation_from_app_handle,
    },
    get_feature_config, AppState,
};

/// Format size in bytes to human-readable format
fn format_size_bytes(bytes: usize) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    const THRESHOLD: f64 = 1024.0;

    if bytes == 0 {
        return "0B".to_string();
    }

    let bytes_f = bytes as f64;
    let mut size = bytes_f;
    let mut unit_index = 0;

    while size >= THRESHOLD && unit_index < UNITS.len() - 1 {
        size /= THRESHOLD;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{}{}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1}{}", size, UNITS[unit_index])
    }
}

/// Macro to check if authoring feature is enabled
macro_rules! check_feature_enabled {
    () => {
        if !get_feature_config().authoring {
            log::warn!("Authoring feature is disabled");
            return Err("Authoring feature is disabled".to_string());
        }
    };
}

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
            (format!("{}", processor_error), 1, 1, 1, 1)
        }
        LangError::Middleware(middleware_error_box) => {
            let middleware_error = &**middleware_error_box;
            (format!("{}", middleware_error), 1, 1, 1, 1)
        }
        LangError::Frontend(frontend_error_box) => {
            let frontend_error = &**frontend_error_box;
            (format!("{}", frontend_error), 1, 1, 1, 1)
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
    check_feature_enabled!();
    let app_state = state.lock().await;

    store::get_default_private_key_info(&app_state.db)
        .await
        .map_err(|e| format!("Failed to get private key info: {}", e))
}

/// Sign a POD with the given key-value pairs
#[tauri::command]
pub async fn sign_pod(
    state: State<'_, Mutex<AppState>>,
    serialized_pod_values: String,
) -> Result<String, String> {
    check_feature_enabled!();
    let app_state = state.lock().await;
    let app_handle = app_state.app_handle.clone();

    let kvs: HashMap<String, PodValue> =
        serde_json::from_str(&serialized_pod_values).map_err(|e| {
            let error_msg = format!("Failed to parse serialized pod values: {}", e);
            let app_handle_clone = app_handle.clone();
            tokio::spawn(async move {
                log_error_event_from_app_handle(
                    &app_handle_clone,
                    format!("❌ Signing failed: Invalid POD values format"),
                )
                .await;
            });
            error_msg
        })?;

    let entry_count = kvs.len();
    let params = Params::default();
    let mut builder = SignedPodBuilder::new(&params);
    for (key, value) in kvs {
        builder.insert(key, value);
    }

    // Get default private key (auto-created if needed)
    let private_key = match store::get_default_private_key(&app_state.db).await {
        Ok(key) => key,
        Err(e) => {
            let error_msg = format!("Failed to get private key: {}", e);
            let app_handle_clone = app_handle.clone();
            tokio::spawn(async move {
                log_error_event_from_app_handle(
                    &app_handle_clone,
                    format!("❌ Signing failed: Private key not available"),
                )
                .await;
            });
            return Err(error_msg);
        }
    };

    let mut signer = Signer(private_key);

    match builder.sign(&mut signer) {
        Ok(signed_pod) => {
            // Get POD ID and size for logging
            let pod_id = signed_pod.id().0.encode_hex::<String>();
            let pod_id_short = pod_id[..8.min(pod_id.len())].to_string();
            let size_bytes = serde_json::to_string(&signed_pod).unwrap_or_default().len();
            let size_str = format_size_bytes(size_bytes);

            // Log successful signing
            let sign_msg = format!(
                "✏️ POD signed via GUI: {} entries → {} ({})",
                entry_count, pod_id_short, size_str
            );
            let app_handle_clone = app_handle.clone();
            tokio::spawn(async move {
                log_pod_operation_from_app_handle(&app_handle_clone, sign_msg).await;
            });

            Ok(serde_json::to_string(&signed_pod).unwrap())
        }
        Err(e) => {
            let error_msg = format!("Failed to sign pod: {}", e);
            let app_handle_clone = app_handle.clone();
            tokio::spawn(async move {
                log_error_event_from_app_handle(
                    &app_handle_clone,
                    format!("❌ Signing failed: {} entries ({})", entry_count, e),
                )
                .await;
            });
            Err(error_msg)
        }
    }
}

// =============================================================================
// Editor Commands
// =============================================================================

/// Validate Podlang code for syntax and semantic errors
#[tauri::command]
pub async fn validate_code_command(code: String) -> Result<ValidateCodeResponse, String> {
    check_feature_enabled!();

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
            log::debug!("Validation error: {:?}", lang_error);
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
    check_feature_enabled!();

    log::debug!(
        "Executing code (mock: {}): {:?}",
        mock,
        code.chars().take(50).collect::<String>()
    );

    let app_state = state.lock().await;
    let app_handle = app_state.app_handle.clone();

    pest::set_error_detail(true);
    let params = Params::default();

    // Parse the code first
    let processed_output = match lang::parse(&code, &params, &[]) {
        Ok(output) => output,
        Err(e) => {
            log::error!("Failed to parse Podlang code: {:?}", e);
            let app_handle_clone = app_handle.clone();
            tokio::spawn(async move {
                log_error_event_from_app_handle(
                    &app_handle_clone,
                    format!("❌ Podlang execution failed: Parse error"),
                )
                .await;
            });
            return Err(format!("Parse error: {}", e));
        }
    };

    if processed_output.request_templates.is_empty() {
        let app_handle_clone = app_handle.clone();
        tokio::spawn(async move {
            log_error_event_from_app_handle(
                &app_handle_clone,
                format!("❌ Podlang execution failed: No POD Request found"),
            )
            .await;
        });
        return Err("Program does not contain a POD Request".to_string());
    }

    // Get all PODs from all spaces
    let all_pod_infos = store::list_all_pods(&app_state.db)
        .await
        .map_err(|e| format!("Failed to list PODs: {}", e))?;

    if all_pod_infos.is_empty() {
        log::warn!("No PODs found for execution. Proceeding with empty facts.");
    }

    let mut owned_signed_pods: Vec<SignedPod> = Vec::new();
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
                owned_signed_pods.push(SignedPod::try_from(helper).unwrap());
            }
            PodData::Main(helper) => match MainPod::try_from(helper) {
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

    let mut all_pods_for_facts: Vec<IndexablePod> = Vec::new();

    for signed_pod_ref in &owned_signed_pods {
        // If not in mock mode, Signed PODs must be of type Signed.
        if !mock && signed_pod_ref.pod.pod_type().0 != PodType::Signed as usize {
            continue;
        }
        all_pods_for_facts.push(IndexablePod::signed_pod(signed_pod_ref));
    }

    for main_pod_ref in &owned_main_pods {
        // If not in mock mode, Main PODs must be of type Main.
        if !mock && main_pod_ref.pod.pod_type().0 != PodType::Main as usize {
            continue;
        }
        all_pods_for_facts.push(IndexablePod::main_pod(main_pod_ref));
    }

    let request_templates = processed_output.request_templates;

    // Solve the query
    let (proof, _) =
        match pod2_solver::solve(&request_templates, &all_pods_for_facts, MetricsLevel::None) {
            Ok(solution) => solution,
            Err(e) => {
                log::error!("Solver error: {:?}", e);
                let app_handle_clone = app_handle.clone();
                tokio::spawn(async move {
                    log_error_event_from_app_handle(
                        &app_handle_clone,
                        format!("❌ Podlang execution failed: Solver error"),
                    )
                    .await;
                });
                return Err(format!("Solver error: {}", e));
            }
        };

    let (pod_ids, ops) = proof.to_inputs();
    let operation_count = ops.len(); // Get count before move

    // Choose VD set based on mock mode
    #[allow(clippy::borrow_interior_mutable_const)]
    let vd_set = if mock { &MOCK_VD_SET } else { &*DEFAULT_VD_SET };

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

    // Create prover based on mock mode
    let prover: Box<dyn PodProver> = if mock {
        Box::new(MockProver {})
    } else {
        Box::new(Prover {})
    };

    let result_main_pod = builder.prove(&*prover, &params).unwrap();

    // Log successful execution
    let main_pod_id = result_main_pod.id().0.encode_hex::<String>();
    let pod_id_short = main_pod_id[..8.min(main_pod_id.len())].to_string();
    let input_count = all_pods_for_facts.len();

    let operation_text = if operation_count == 1 {
        "operation"
    } else {
        "operations"
    };
    let execution_msg = if mock {
        format!(
            "⚡ Podlang executed (mock): {} inputs → {} ({} {})",
            input_count, pod_id_short, operation_count, operation_text
        )
    } else {
        format!(
            "⚡ Podlang executed: {} inputs → {} ({} {})",
            input_count, pod_id_short, operation_count, operation_text
        )
    };

    let app_handle_clone = app_handle.clone();
    tokio::spawn(async move {
        log_pod_operation_from_app_handle(&app_handle_clone, execution_msg).await;
    });

    let result = ExecuteCodeResponse {
        main_pod: result_main_pod,
        diagram: pod2_solver::vis::mermaid_markdown(&proof),
    };

    Ok(result)
}
