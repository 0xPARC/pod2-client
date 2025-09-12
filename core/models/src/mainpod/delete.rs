//! Delete verification MainPod operations
//!
//! This module handles cryptographic verification of document deletion requests
//! using MainPods that prove identity and ownership.
//!
//! The delete verification process ensures:
//! 1. User identity is verified via signed identity pod
//! 2. User has ownership/authority over the document being deleted
//! 3. Document exists and is accessible for deletion
//! 4. All cryptographic proofs are valid

use pod_utils::prover_setup::PodNetProverSetup;
use pod2::{
    frontend::{MainPod, SignedDict},
    lang::parse,
    middleware::{Params, Value},
};
use pod2_new_solver::{
    Engine, EngineConfigBuilder, ImmutableEdbBuilder, OpRegistry,
    build_pod_from_answer_top_level_public,
};

use super::{MainPodError, MainPodResult};

/// Datalog predicate for delete verification
pub fn get_delete_verification_predicate() -> String {
    r#"
        identity_verified(username, identity_pod) = AND(
            Equal(identity_pod["username"], username)
        )

        document_verified(data, timestamp_pod, private: identity_pod, document_pod) = AND(
            Equal(document_pod["request_type"], "delete")
            Equal(document_pod["data"], data)
            Equal(document_pod["timestamp_pod"], timestamp_pod)
            SignedBy(document_pod, identity_pod["user_public_key"])
        )

        delete_verified(username, data, identity_server_pk, timestamp_pod, private: identity_pod, document_pod) = AND(
            identity_verified(username, identity_pod)
            document_verified(data, timestamp_pod)
            SignedBy(identity_pod, identity_server_pk)
        )
    "#.to_string()
}

/// Parameters for delete verification proof generation
pub struct DeleteProofParams<'a> {
    pub identity_pod: &'a SignedDict,
    pub document_pod: &'a SignedDict,
    pub timestamp_pod: &'a SignedDict,
    pub use_mock_proofs: bool,
}

pub fn prove_delete(params: DeleteProofParams) -> MainPodResult<MainPod> {
    // Extract required values from pods
    let username = params
        .identity_pod
        .get("username")
        .ok_or(MainPodError::MissingField {
            pod_type: "Identity",
            field: "username",
        })?;
    let identity_server_pk = params.identity_pod.public_key;
    let data = params
        .document_pod
        .get("data")
        .ok_or(MainPodError::MissingField {
            pod_type: "Document",
            field: "data",
        })?
        .clone();
    let timestamp_pod_id = Value::from(params.timestamp_pod.dict.commitment());

    // Start with the existing predicate definitions and append REQUEST
    let mut query = get_delete_verification_predicate();

    query.push_str(&format!(
        r#"

        REQUEST(
            delete_verified({username}, {data}, {identity_server_pk}, {timestamp_pod_id})
        )
        "#
    ));
    println!("QUERY: {query}");

    // Parse the complete query
    let pod_params = Params::default();
    let request = parse(&query, &pod_params, &[])
        .map_err(|e| MainPodError::ProofGeneration(format!("Parse error: {e:?}")))?;

    let edb = ImmutableEdbBuilder::new()
        .add_signed_dict(params.identity_pod.clone())
        .add_signed_dict(params.document_pod.clone())
        .add_signed_dict(params.timestamp_pod.clone())
        .build();

    let reg = OpRegistry::default();
    let config = EngineConfigBuilder::new().from_params(&pod_params).build();
    let mut engine = Engine::with_config(&reg, &edb, config);
    engine.load_processed(&request);
    engine
        .run()
        .map_err(|e| MainPodError::ProofGeneration(format!("Solver error: {e:?}")))?;

    let pod_params = PodNetProverSetup::get_params();
    let (vd_set, prover) = PodNetProverSetup::create_prover_setup(params.use_mock_proofs)
        .map_err(MainPodError::ProofGeneration)?;

    let main_pod = build_pod_from_answer_top_level_public(
        &engine.answers[0],
        &pod_params,
        vd_set,
        |b| b.prove(&*prover).map_err(|e| e.to_string()),
        &edb,
    )
    .map_err(|e| MainPodError::ProofGeneration(format!("Pod build error: {e:?}")))?;

    println!("GOT MAINPOD: {main_pod}");
    main_pod.pod.verify().map_err(|e| {
        MainPodError::ProofGeneration(format!("MainPod verification failed: {e:?}"))
    })?;

    Ok(main_pod)
}

/// Verify delete request MainPod using the solver
pub fn verify_delete_verification_with_solver(
    main_pod: &MainPod,
    expected_username: &str,
    expected_data: &Value,
    expected_identity_server_pk: &Value,
    expected_timestamp_pod: &SignedDict,
) -> MainPodResult<()> {
    // Start with the existing predicate definitions and append REQUEST
    let mut query = get_delete_verification_predicate().to_string();

    let username_value = Value::from(expected_username);
    let timestamp_pod_id_value = Value::from(expected_timestamp_pod.dict.commitment());

    query.push_str(&format!(
        r#"

        REQUEST(
            delete_verified({username_value}, {expected_data}, {expected_identity_server_pk}, {timestamp_pod_id_value})
        )
        "#
    ));
    println!("DELETE QUERY: {query}");

    // Parse the complete query
    let pod_params = Params::default();
    let request = parse(&query, &pod_params, &[])
        .map_err(|e| MainPodError::Verification(format!("Parse error: {e:?}")))?
        .request;

    request
        .exact_match_pod(&*main_pod.pod)
        .map_err(|e| MainPodError::Verification(format!("Exact match pod error: {e:?}")))?;

    println!("GOT DELETE PROOF: {main_pod}");

    log::info!("✓ Delete verification succeeded for user {expected_username}");
    Ok(())
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_delete_predicate_parsing() {
        let params = PodNetProverSetup::get_params();
        let predicate_input = get_delete_verification_predicate();
        let result = pod2::lang::parse(&predicate_input, &params, &[]);

        assert!(result.is_ok(), "Delete predicate should parse successfully");

        let batch = result.unwrap().custom_batch;
        assert!(
            batch.predicate_ref_by_name("delete_verified").is_some(),
            "delete_verified predicate should exist"
        );
        assert!(
            batch.predicate_ref_by_name("identity_verified").is_some(),
            "identity_verified predicate should exist"
        );
    }
}
