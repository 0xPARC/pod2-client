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

use pod_utils::{ValueExt, prover_setup::PodNetProverSetup};
use pod2::{
    frontend::{MainPod, MainPodBuilder, SignedPod},
    lang::parse,
    middleware::{Hash, KEY_SIGNER, KEY_TYPE, Key, Params, PodType, Value},
};
use pod2_solver::{SolverContext, db::IndexablePod, metrics::MetricsLevel, solve};

use super::{MainPodError, MainPodResult};

/// Datalog predicate for delete verification
pub fn get_delete_verification_predicate() -> String {
    format!(
        r#"
        identity_verified(username, private: identity_pod) = AND(
            Equal(?identity_pod["{key_type}"], {signed_pod_type})
            Equal(?identity_pod["username"], ?username)
        )

        document_verified(data, timestamp_pod, private: identity_pod, document_pod) = AND(
            Equal(?document_pod["request_type"], "delete")
            Equal(?document_pod["data"], ?data)
            Equal(?document_pod["timestamp_pod"], ?timestamp_pod)
            Equal(?document_pod["{key_signer}"], ?identity_pod["user_public_key"])
        )

        delete_verified(username, data, identity_server_pk, timestamp_pod, private: identity_pod, document_pod) = AND(
            identity_verified(?username)
            document_verified(?data, ?timestamp_pod)
            Equal(?identity_pod["{key_signer}"], ?identity_server_pk)
        )
    "#,
        key_type = KEY_TYPE,
        key_signer = KEY_SIGNER,
        signed_pod_type = PodType::Signed as usize,
    )
}

/// Parameters for delete verification proof generation
pub struct DeleteProofParams<'a> {
    pub identity_pod: &'a SignedPod,
    pub document_pod: &'a SignedPod,
    pub timestamp_pod: &'a SignedPod,
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
    let identity_server_pk =
        params
            .identity_pod
            .get(KEY_SIGNER)
            .ok_or(MainPodError::MissingField {
                pod_type: "Identity",
                field: "identity_server_pk",
            })?;
    let data = params
        .document_pod
        .get("data")
        .ok_or(MainPodError::MissingField {
            pod_type: "Document",
            field: "data",
        })?
        .clone();
    let timestamp_pod_id = Value::from(params.timestamp_pod.id());

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
        .map_err(|e| MainPodError::ProofGeneration(format!("Parse error: {e:?}")))?
        .request;

    // Provide all three pods as facts
    let pods = [
        IndexablePod::signed_pod(params.identity_pod),
        IndexablePod::signed_pod(params.document_pod),
    ];

    // Let the solver find the proof
    let context = SolverContext::new(&pods, &[]);
    let (proof, _metrics) = solve(request.templates(), &context, MetricsLevel::Counters)
        .map_err(|e| MainPodError::ProofGeneration(format!("Solver error: {e:?}")))?;

    let pod_params = PodNetProverSetup::get_params();
    let (vd_set, prover) = PodNetProverSetup::create_prover_setup(params.use_mock_proofs)
        .map_err(MainPodError::ProofGeneration)?;

    let mut builder = MainPodBuilder::new(&pod_params, vd_set);

    let (pod_ids, ops) = proof.to_inputs();

    for (op, public) in ops {
        if public {
            builder
                .pub_op(op)
                .map_err(|e| MainPodError::ProofGeneration(format!("Builder error: {e:?}")))?;
        } else {
            builder
                .priv_op(op)
                .map_err(|e| MainPodError::ProofGeneration(format!("Builder error: {e:?}")))?;
        }
    }

    // Add all the pods that were referenced in the proof
    for pod_id in pod_ids {
        if params.identity_pod.id() == pod_id {
            builder.add_signed_pod(params.identity_pod);
        } else if params.document_pod.id() == pod_id {
            builder.add_signed_pod(params.document_pod);
        }
    }

    let main_pod = builder
        .prove(&*prover, &pod_params)
        .map_err(|e| MainPodError::ProofGeneration(format!("Prove error: {e:?}")))?;

    println!("GOT MAINPOD: {}", main_pod);
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
    expected_timestamp_pod: &SignedPod,
) -> MainPodResult<()> {
    // Start with the existing predicate definitions and append REQUEST
    let mut query = get_delete_verification_predicate().to_string();

    let username_value = Value::from(expected_username);
    let timestamp_pod_id_value = Value::from(expected_timestamp_pod.id());

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

    // Provide the main pod as fact
    let pods = [IndexablePod::main_pod(main_pod)];

    // Let the solver find the proof
    let context = SolverContext::new(&pods, &[]);
    let (proof, _metrics) = solve(request.templates(), &context, MetricsLevel::Counters)
        .map_err(|e| MainPodError::Verification(format!("Solver error: {e:?}")))?;
    println!("GOT DELETE PROOF: {proof}");

    log::info!(
        "✓ Delete verification succeeded for user {}",
        expected_username
    );
    Ok(())
}

/// Extract delete arguments from MainPod (simplified version for now)
pub fn extract_delete_args(main_pod: &MainPod) -> MainPodResult<(String, i64, Value)> {
    // For now, return a placeholder - this would need proper implementation
    // using the same pattern as publish verification
    Err(MainPodError::Verification(
        "extract_delete_args not yet implemented".to_string(),
    ))
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
