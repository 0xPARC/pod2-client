use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, LazyLock},
};

use anyhow::Result;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use hex::ToHex;
use log::info;
use num::BigUint;
use pod2::{
    backends::plonky2::{
        mainpod::Prover, mock::mainpod::MockProver, primitives::ec::schnorr::SecretKey,
        signedpod::Signer,
    },
    frontend::{MainPod, MainPodBuilder, SerializedSignedPod, SignedPod, SignedPodBuilder},
    lang::{self, parser, LangError},
    middleware::{
        containers::Set, Params, PodId, PodProver, PodType, VDSet, Value as PodValue,
        DEFAULT_VD_SET,
    },
};
use pod2_solver::{self, db::IndexablePod, error::SolverError, metrics::MetricsLevel};
use serde::{Deserialize, Serialize};

use crate::{
    api_types::{
        Diagnostic, DiagnosticSeverity, ExecuteCodeRequest, PodData, ValidateCodeRequest,
        ValidateCodeResponse,
    },
    db::{store, Db},
};

#[allow(clippy::declare_interior_mutable_const)]
pub const MOCK_VD_SET: LazyLock<VDSet> = LazyLock::new(|| VDSet::new(6, &[]).unwrap());

#[derive(Serialize, Deserialize)]
pub struct ExecuteResult {
    main_pod: MainPod,
    diagram: String,
}

// --- Playground API Handlers ---

pub async fn validate_code_handler(
    Json(payload): Json<ValidateCodeRequest>,
) -> Result<Json<ValidateCodeResponse>, PlaygroundApiError> {
    log::debug!(
        "Received validate_code request for code starting with: {:?}",
        payload.code.chars().take(50).collect::<String>()
    );

    let params = Params::default();
    pest::set_error_detail(true);
    match lang::parse(&payload.code, &params, &[]) {
        Ok(_) => Ok(Json(ValidateCodeResponse {
            diagnostics: vec![],
        })),
        Err(lang_error) => {
            println!("LangError: {:?}", lang_error);
            let diagnostics = lang_error_to_diagnostics(&lang_error);
            Ok(Json(ValidateCodeResponse { diagnostics }))
        }
    }
}

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
        start_line,
        start_column: start_col,
        end_line,
        end_column: end_col,
    });

    diagnostics
}

pub async fn execute_code_handler(
    State(db): State<Arc<Db>>,
    Json(payload): Json<ExecuteCodeRequest>,
) -> Result<Json<ExecuteResult>, PlaygroundApiError> {
    log::debug!(
        "Received execute_code request for space '{}' with code starting with: {:?}",
        payload.space_id,
        payload.code.chars().take(50).collect::<String>()
    );

    pest::set_error_detail(true);
    let params = Params {
        // Currently the circuit uses random access that only supports vectors of length 64.
        // With max_input_main_pods=3 we need random access to a vector of length 73.
        // max_input_recursive_pods: 1,
        ..Default::default()
    };

    let processed_output = match lang::parse(&payload.code, &params, &[]) {
        Ok(output) => output,
        Err(e) => {
            log::error!("Failed to parse Podlog code: {:?}", e);
            return Err(PlaygroundApiError::Lang(e));
        }
    };

    if !store::space_exists(&db, &payload.space_id).await? {
        log::warn!("Space '{}' not found for execution", payload.space_id);
        return Err(PlaygroundApiError::NotFound(format!(
            "Space with id '{}' not found for execution.",
            payload.space_id
        )));
    }

    let fetched_pod_infos = store::list_pods(&db, &payload.space_id).await?;

    if fetched_pod_infos.is_empty() {
        log::warn!(
            "No pods found in space '{}' for execution. Proceeding with empty facts.",
            payload.space_id
        );
    }

    let mut owned_signed_pods: Vec<SignedPod> = Vec::new();
    let mut owned_main_pods: Vec<MainPod> = Vec::new();

    for pod_info in fetched_pod_infos {
        // Sanity check: Ensure the pod_type string from DB matches the PodData enum variant type
        if pod_info.pod_type != pod_info.data.type_str() {
            log::warn!(
                "Data inconsistency for pod_id '{}' in space '{}' during execution: DB pod_type is '{}' but deserialized PodData is for '{}'. Trusting PodData enum.",
                pod_info.id, payload.space_id, pod_info.pod_type, pod_info.data.type_str()
            );
            // If they mismatch, we should probably trust the actual data content (the enum variant)
            // but it indicates a potential issue elsewhere (e.g., during import or manual DB edit).
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
                        payload.space_id,
                        e
                    );
                    return Err(PlaygroundApiError::Internal(anyhow::anyhow!(
                        "Failed to process stored pod data for pod id {} in space {}: {:?}",
                        pod_info.id,
                        payload.space_id,
                        e
                    )));
                }
            },
        }
    }

    let mut all_pods_for_facts: Vec<IndexablePod> = Vec::new();
    let mut original_signed_pods: HashMap<PodId, &SignedPod> = HashMap::new();
    let mut original_main_pods: HashMap<PodId, &MainPod> = HashMap::new();

    for signed_pod_ref in &owned_signed_pods {
        // If not in mock mode, Signed PODs must be of type Signed.
        if !payload.mock && signed_pod_ref.pod.pod_type().0 != PodType::Signed as usize {
            continue;
        }
        all_pods_for_facts.push(IndexablePod::signed_pod(signed_pod_ref));
        original_signed_pods.insert(signed_pod_ref.id(), signed_pod_ref);
    }

    for main_pod_ref in &owned_main_pods {
        // If not in mock mode, Main PODs must be of type Main.
        if !payload.mock && main_pod_ref.pod.pod_type().0 != PodType::Main as usize {
            continue;
        }
        all_pods_for_facts.push(IndexablePod::main_pod(main_pod_ref));
        original_main_pods.insert(main_pod_ref.id(), main_pod_ref);
    }

    // let initial_facts = facts_from_pods(&all_pods_for_facts);
    // let custom_definitions =
    //     custom_definitions_from_batches(&[processed_output.custom_batch], &params);
    let request_templates = processed_output.request_templates;

    let (proof, _) =
        match pod2_solver::solve(&request_templates, &all_pods_for_facts, MetricsLevel::None) {
            Ok(solution) => solution,
            Err(e) => {
                log::error!("Solver error: {:?}", e);
                return Err(PlaygroundApiError::Solver(e));
            }
        };

    let (pod_ids, ops) = proof.to_inputs();

    let vd_set = if payload.mock {
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
    let prover: Box<dyn PodProver> = if payload.mock {
        Box::new(MockProver {})
    } else {
        Box::new(Prover {})
    };
    let result_main_pod = builder.prove(&*prover, &params).unwrap();

    let result = ExecuteResult {
        main_pod: result_main_pod,
        diagram: pod2_solver::vis::mermaid_markdown(&proof),
    };

    Ok(Json(result))
}

#[derive(Debug)]
pub enum PlaygroundApiError {
    ValidationFailed(ValidateCodeResponse),
    Solver(SolverError),
    Lang(LangError),
    Internal(anyhow::Error),
    NotFound(String),
}

impl IntoResponse for PlaygroundApiError {
    fn into_response(self) -> axum::response::Response {
        match self {
            PlaygroundApiError::ValidationFailed(validation_response) => {
                log::warn!("Validation failed: {:?}", validation_response.diagnostics);
                (StatusCode::BAD_REQUEST, Json(validation_response)).into_response()
            }
            PlaygroundApiError::Solver(solver_error) => {
                log::error!("Solver error during execution: {:#?}", solver_error);
                let (status_code, error_message_str) =
                    (StatusCode::INTERNAL_SERVER_ERROR, "Proof generation failed");
                let error_body = serde_json::json!({
                    "error": error_message_str,
                    "details": format!("{:?}", solver_error)
                });
                (status_code, Json(error_body)).into_response()
            }
            PlaygroundApiError::Lang(lang_error) => {
                log::error!("Language processing error: {:#?}", lang_error);
                let diagnostics = lang_error_to_diagnostics(&lang_error);
                let error_body = serde_json::json!({
                    "error": "Language processing error",
                    "diagnostics": diagnostics
                });
                (StatusCode::BAD_REQUEST, Json(error_body)).into_response()
            }
            PlaygroundApiError::Internal(err) => {
                log::error!("Internal server error: {:#}", err);
                let error_body = serde_json::json!({ "error": "Internal server error" });
                (StatusCode::INTERNAL_SERVER_ERROR, Json(error_body)).into_response()
            }
            PlaygroundApiError::NotFound(message) => {
                log::warn!("Playground API resource not found: {}", message);
                let error_body = serde_json::json!({
                    "error": "Not Found",
                    "message": message
                });
                (StatusCode::NOT_FOUND, Json(error_body)).into_response()
            }
        }
    }
}

impl From<LangError> for PlaygroundApiError {
    fn from(err: LangError) -> Self {
        PlaygroundApiError::Lang(err)
    }
}

impl From<SolverError> for PlaygroundApiError {
    fn from(err: SolverError) -> Self {
        PlaygroundApiError::Solver(err)
    }
}

impl From<anyhow::Error> for PlaygroundApiError {
    fn from(err: anyhow::Error) -> Self {
        PlaygroundApiError::Internal(err)
    }
}

pub async fn setup_zukyc_space(db: &Db) -> anyhow::Result<()> {
    let space_id = "zukyc";

    if store::space_exists(db, space_id).await? {
        info!("Space '{}' already exists. Skipping setup.", space_id);
        return Ok(());
    }

    info!("Setting up space '{}' with Zukyc sample pods...", space_id);
    store::create_space(db, space_id).await?;

    let params_for_test = Params::default();
    let mut gov_signer = Signer(SecretKey(BigUint::from(1u32)));
    let mut pay_signer = Signer(SecretKey(BigUint::from(2u32)));
    let mut sanction_signer = Signer(SecretKey(BigUint::from(3u32)));

    let mut gov_id_builder = SignedPodBuilder::new(&params_for_test);
    gov_id_builder.insert("idNumber", "4242424242");
    gov_id_builder.insert("dateOfBirth", 1169909384);
    gov_id_builder.insert("socialSecurityNumber", "G2121210");

    match gov_id_builder.sign(&mut gov_signer) {
        Ok(gov_id_pod_signed) => {
            let gov_id_helper: SerializedSignedPod = gov_id_pod_signed.clone().into();
            let gov_pod_id_str: String = gov_id_pod_signed.id().0.encode_hex();
            let pod_data = PodData::Signed(gov_id_helper);
            if let Err(e) = store::import_pod(
                db,
                &gov_pod_id_str,
                "signed",
                &pod_data,
                Some("Gov ID"),
                space_id,
            )
            .await
            {
                log::error!(
                    "Failed to insert Gov ID pod into Zukyc space '{}': {}",
                    space_id,
                    e
                );
            }
        }
        Err(e) => {
            log::error!("Failed to sign Gov ID pod for Zukyc setup: {}", e);
        }
    }

    let mut pay_stub_builder = SignedPodBuilder::new(&params_for_test);
    pay_stub_builder.insert("socialSecurityNumber", "G2121210");
    pay_stub_builder.insert("startDate", 1706367566);
    match pay_stub_builder.sign(&mut pay_signer) {
        Ok(pay_stub_pod_signed) => {
            let pay_stub_helper: SerializedSignedPod = pay_stub_pod_signed.clone().into();
            let pay_pod_id_str: String = pay_stub_pod_signed.id().0.encode_hex();
            let pod_data = PodData::Signed(pay_stub_helper);
            if let Err(e) = store::import_pod(
                db,
                &pay_pod_id_str,
                "signed",
                &pod_data,
                Some("Pay Stub"),
                space_id,
            )
            .await
            {
                log::error!(
                    "Failed to insert Pay Stub pod into Zukyc space '{}': {}",
                    space_id,
                    e
                );
            }
        }
        Err(e) => {
            log::error!("Failed to sign Pay Stub pod for Zukyc setup: {}", e);
        }
    }

    let sanctions_values_set: HashSet<PodValue> =
        ["A343434340"].iter().map(|s| PodValue::from(*s)).collect();

    match Set::new(
        params_for_test.max_depth_mt_containers,
        sanctions_values_set,
    ) {
        Ok(sanction_set_typed) => {
            let sanction_set_val = PodValue::from(sanction_set_typed);
            let mut sanction_list_builder = SignedPodBuilder::new(&params_for_test);
            sanction_list_builder.insert("sanctionList", sanction_set_val);
            match sanction_list_builder.sign(&mut sanction_signer) {
                Ok(sanction_list_pod_signed) => {
                    let sanction_list_helper: SerializedSignedPod =
                        sanction_list_pod_signed.clone().into();
                    let sanction_pod_id_str: String = sanction_list_pod_signed.id().0.encode_hex();
                    let pod_data = PodData::Signed(sanction_list_helper);
                    if let Err(e) = store::import_pod(
                        db,
                        &sanction_pod_id_str,
                        "signed",
                        &pod_data,
                        Some("Sanctions List"),
                        space_id,
                    )
                    .await
                    {
                        log::error!(
                            "Failed to insert Sanctions List pod into Zukyc space '{}': {}",
                            space_id,
                            e
                        );
                    }
                }
                Err(e) => {
                    log::error!("Failed to sign Sanctions List pod for Zukyc setup: {}", e);
                }
            }
        }
        Err(e) => {
            log::error!("Failed to create sanction set for Zukyc setup: {}", e);
        }
    }

    info!(
        "Zukyc space setup attempt complete for space '{}'. Check logs for any errors.",
        space_id
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{io::Write, sync::Once};

    use axum_test::TestServer;
    use env_logger::Builder;
    use pod2::backends::plonky2::mock::signedpod::MockSigner;
    use serde_json::json;

    use super::*; // Imports handlers, PlaygroundApiError, etc.
    use crate::{db, routes::create_router};

    static INIT: Once = Once::new();
    fn setup_test_logging() {
        INIT.call_once(|| {
            Builder::from_default_env()
                .format(|buf, record| {
                    writeln!(
                        buf,
                        "{} [{}] - {}",
                        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                        record.level(),
                        record.args()
                    )
                })
                .filter(None, log::LevelFilter::Debug)
                .init();
        });
    }

    async fn create_playground_server() -> TestServer {
        setup_test_logging();
        let db = Arc::new(
            db::Db::new(None, &db::MIGRATIONS)
                .await
                .expect("Failed to init db for test"),
        );
        let router = create_router(db);
        TestServer::new(router).unwrap()
    }

    async fn create_playground_server_with_db() -> (TestServer, Arc<Db>) {
        setup_test_logging();
        let db = Arc::new(
            db::Db::new(None, &db::MIGRATIONS)
                .await
                .expect("Failed to init db for test"),
        );
        let router = create_router(db.clone());
        (TestServer::new(router).unwrap(), db)
    }

    #[ignore]
    #[tokio::test]
    async fn test_execute_code_with_space_success() {
        let (server, db) = create_playground_server_with_db().await;
        let space_id = "exec-space-success-real";

        // Create the space directly in DB for test setup
        store::create_space(&db, space_id)
            .await
            .expect("Failed to create space for test");

        let params_for_test = Params::default();
        let mut gov_signer = MockSigner { pk: "gov".into() };
        let mut pay_signer = MockSigner { pk: "pay".into() };
        let mut sanction_signer = MockSigner {
            pk: "sanction".into(),
        };

        let mut gov_id_builder = SignedPodBuilder::new(&params_for_test);
        gov_id_builder.insert("idNumber", "4242424242");
        gov_id_builder.insert("dateOfBirth", 1169909384);
        gov_id_builder.insert("socialSecurityNumber", "G2121210");
        let gov_id_pod_signed = gov_id_builder.sign(&mut gov_signer).unwrap();
        let gov_id_helper: SerializedSignedPod = gov_id_pod_signed.clone().into();
        let gov_pod_id_str: String = gov_id_pod_signed.id().0.encode_hex();

        // Import pod directly into DB for test setup
        let pod_data_gov = PodData::Signed(gov_id_helper);
        store::import_pod(
            &db,
            &gov_pod_id_str,
            "signed",
            &pod_data_gov,
            Some("Gov ID"),
            space_id,
        )
        .await
        .expect("Failed to import Gov ID pod for test");

        let mut pay_stub_builder = SignedPodBuilder::new(&params_for_test);
        pay_stub_builder.insert("socialSecurityNumber", "G2121210");
        pay_stub_builder.insert("startDate", 1706367566);
        let pay_stub_pod_signed = pay_stub_builder.sign(&mut pay_signer).unwrap();
        let pay_stub_helper: SerializedSignedPod = pay_stub_pod_signed.clone().into();
        let pay_pod_id_str: String = pay_stub_pod_signed.id().0.encode_hex();

        let pod_data_pay = PodData::Signed(pay_stub_helper);
        store::import_pod(
            &db,
            &pay_pod_id_str,
            "signed",
            &pod_data_pay,
            Some("Pay Stub"),
            space_id,
        )
        .await
        .expect("Failed to import Pay Stub pod for test");

        let sanctions_values_set: HashSet<PodValue> =
            ["A343434340"].iter().map(|s| PodValue::from(*s)).collect();
        let sanction_set_val = PodValue::from(
            Set::new(
                params_for_test.max_depth_mt_containers,
                sanctions_values_set,
            )
            .unwrap(),
        );
        let mut sanction_list_builder = SignedPodBuilder::new(&params_for_test);
        sanction_list_builder.insert("sanctionList", sanction_set_val);
        let sanction_list_pod_signed = sanction_list_builder.sign(&mut sanction_signer).unwrap();
        let sanction_list_helper: SerializedSignedPod = sanction_list_pod_signed.clone().into();
        let sanction_pod_id_str: String = sanction_list_pod_signed.id().0.encode_hex();

        let pod_data_sanction = PodData::Signed(sanction_list_helper);
        store::import_pod(
            &db,
            &sanction_pod_id_str,
            "signed",
            &pod_data_sanction,
            Some("Sanctions List"),
            space_id,
        )
        .await
        .expect("Failed to import Sanctions List pod for test");

        let const_18y = 1169909388;
        let const_1y = 1706367566;

        let valid_zukyc_podlog = format!(
            r#"
        REQUEST(
            NotContains(?sanctions["sanctionList"], ?gov["idNumber"])
            Lt(?gov["dateOfBirth"], {})
            Equal(?pay["startDate"], {})
            Equal(?gov["socialSecurityNumber"], ?pay["socialSecurityNumber"])
        )
        "#,
            const_18y, const_1y
        );

        let request_payload = json!({
            "code": valid_zukyc_podlog,
            "space_id": space_id
        });

        let response = server.post("/api/execute").json(&request_payload).await;
        println!("response: {:#?}", response.text());
        assert_eq!(
            response.status_code(),
            StatusCode::OK,
            "Response body: {:?}",
            response.text()
        );

        let result: ExecuteResult = response.json();
        assert_eq!(
            result.main_pod.public_statements.len(),
            5,
            "Expected 5 public statements (4 requests + 1 _type)"
        );
        assert!(result.main_pod.pod.verify().is_ok());
    }

    #[tokio::test]
    async fn test_validate_code_valid_code() {
        let server = create_playground_server().await;

        let valid_podlog_code = r#"
        is_older(PersonA, PersonB) = AND(
            Gt(?PersonA["age"], ?PersonB["age"])
        )
        REQUEST(
            is_older(?Alice, ?Bob)
        )
        "#;

        let request_payload = json!({
            "code": valid_podlog_code
        });

        let response = server.post("/api/validate").json(&request_payload).await;

        assert_eq!(response.status_code(), StatusCode::OK);
        let response_json: serde_json::Value = response.json();

        let diagnostics = response_json
            .get("diagnostics")
            .expect("Response should have a 'diagnostics' field");
        assert!(
            diagnostics.is_array(),
            "'diagnostics' field should be an array"
        );
        assert!(
            diagnostics.as_array().unwrap().is_empty(),
            "Expected no diagnostics for valid code, got: {:?}",
            diagnostics
        );
    }

    #[tokio::test]
    async fn test_validate_code_parsing_error() {
        let server = create_playground_server().await;

        let invalid_podlog_code = r#"REQEST("#;

        let request_payload = json!({
            "code": invalid_podlog_code
        });

        let response = server.post("/api/validate").json(&request_payload).await;

        assert_eq!(response.status_code(), StatusCode::OK);
        let response_json: serde_json::Value = response.json();

        let diagnostics_val = response_json
            .get("diagnostics")
            .expect("Response should have a 'diagnostics' field");
        assert!(
            diagnostics_val.is_array(),
            "'diagnostics' field should be an array"
        );
        let diagnostics_array = diagnostics_val.as_array().unwrap();

        assert!(
            !diagnostics_array.is_empty(),
            "Expected diagnostics for invalid code, but got none."
        );
        assert_eq!(
            diagnostics_array.len(),
            1,
            "Expected one diagnostic for this specific parse error, got: {:?}",
            diagnostics_array
        );

        let diagnostic_one = &diagnostics_array[0];
        let message = diagnostic_one.get("message").unwrap().as_str().unwrap();

        assert!(message.to_lowercase().contains("expected"), 
                "Diagnostic message '{}' did not contain 'expected'. Actual pest error might be more specific.", message);

        assert_eq!(
            diagnostic_one.get("severity").unwrap().as_str().unwrap(),
            "Error"
        );

        let start_line = diagnostic_one.get("start_line").unwrap().as_u64().unwrap();
        let start_col = diagnostic_one
            .get("start_column")
            .unwrap()
            .as_u64()
            .unwrap();
        assert_eq!(start_line, 1);
        assert_eq!(start_col, 8);
    }

    #[tokio::test]
    async fn test_execute_code_with_space_not_found() {
        let server = create_playground_server().await;
        let space_id = "non-existent-exec-space-for-playground";

        // Ensure the space does NOT exist for this test case.
        // If space is not found, execute_code_handler should return PlaygroundApiError::NotFound.

        let podlog_code = r#"REQUEST(Equal(?gov["idNumber"], ?gov["idNumber"]))"#;

        let request_payload = json!({
            "code": podlog_code,
            "space_id": space_id
        });

        let response = server.post("/api/execute").json(&request_payload).await;
        assert_eq!(
            response.status_code(),
            StatusCode::NOT_FOUND,
            "Response body: {:?}",
            response.text()
        );
        let response_json: serde_json::Value = response.json();
        assert_eq!(
            response_json.get("error").unwrap().as_str().unwrap(),
            "Not Found"
        );
        assert!(response_json
            .get("message")
            .unwrap()
            .as_str()
            .unwrap()
            .contains(&format!(
                "Space with id '{}' not found for execution.",
                space_id
            )));
    }

    #[tokio::test]
    async fn test_setup_zukyc_space_idempotent() {
        let (_server, db) = create_playground_server_with_db().await;

        // Call first time
        setup_zukyc_space(&db)
            .await
            .expect("Zukyc space setup failed");

        // Call second time
        setup_zukyc_space(&db)
            .await
            .expect("Zukyc space setup failed");
    }

    #[tokio::test]
    async fn test_reset_db() {
        let (_server, db) = create_playground_server_with_db().await;

        // 1. Setup zuky's space and add a pod to it.
        setup_zukyc_space(&db)
            .await
            .expect("Zukyc space setup failed");

        // 2. Reset the database
        // ... (implementation of reset_db function)
    }
}
