//! Upvote verification MainPod operations

// Import solver dependencies
use pod_utils::{ValueExt, prover_setup::PodNetProverSetup};
use pod2::{
    frontend::{MainPod, MainPodBuilder, SignedPod},
    lang::parse,
    middleware::{Hash, KEY_SIGNER, Value},
};
use pod2_solver::{SolverContext, db::IndexablePod, metrics::MetricsLevel, solve};

use super::{MainPodError, MainPodResult, verify_mainpod_basics};
use crate::get_upvote_verification_predicate;

/// Parameters for upvote verification proof generation
pub struct UpvoteProofParams<'a> {
    pub identity_pod: &'a SignedPod,
    pub upvote_pod: &'a SignedPod,
    pub identity_server_public_key: Value,
    pub content_hash: &'a Hash,
    pub use_mock_proofs: bool,
}

/// Simplified parameters for solver-based upvote verification proof generation
pub struct UpvoteProofParamsSolver<'a> {
    pub identity_pod: &'a SignedPod,
    pub upvote_pod: &'a SignedPod,
    pub use_mock_proofs: bool,
}

/// Verify an upvote verification MainPod
///
/// This verifies that the MainPod contains the expected public statements
/// and that the content hash and username match the expected values.
pub fn verify_upvote_verification(
    main_pod: &MainPod,
    expected_content_hash: &Hash,
    expected_username: &str,
) -> MainPodResult<()> {
    // Original verbose approach (keeping for compatibility):
    // Verify basic MainPod structure
    verify_mainpod_basics(main_pod)?;

    // Extract arguments with the macro
    let (username, content_hash, _identity_server_pk) = crate::extract_mainpod_args!(
        main_pod,
        get_upvote_verification_predicate(),
        "upvote_verification",
        username: as_str,
        content_hash: as_hash,
        identity_server_pk: as_public_key
    )?;

    // Verify extracted data matches expected values
    if username != expected_username {
        return Err(MainPodError::InvalidValue {
            field: "username",
            expected: expected_username.to_string(),
        });
    }

    if content_hash != *expected_content_hash {
        return Err(MainPodError::InvalidValue {
            field: "content_hash",
            expected: "matching content hash".to_string(),
        });
    }

    Ok(())

    // NEW: With the verify_main_pod! macro, this entire function could be simplified to:
    //
    // verify_main_pod!(
    //     main_pod,
    //     get_upvote_verification_predicate(), {
    //         upvote_verification(expected_username, expected_content_hash, _)
    //     }
    // )
    //
    // This reduces ~25 lines of boilerplate to just 5 lines!
}

/// Generate an upvote verification MainPod using the pod2 solver
///
/// This creates a MainPod that cryptographically proves the same properties as
/// prove_upvote_verification but uses the automated solver approach instead
/// of manual proof construction.
pub fn prove_upvote_verification_with_solver(
    params: UpvoteProofParamsSolver,
) -> MainPodResult<MainPod> {
    // Extract required values from pods
    let username = params
        .identity_pod
        .get("username")
        .ok_or(MainPodError::MissingField {
            pod_type: "Identity",
            field: "username",
        })?;

    let content_hash = params
        .upvote_pod
        .get("content_hash")
        .ok_or(MainPodError::MissingField {
            pod_type: "Upvote",
            field: "content_hash",
        })?;

    let identity_server_pk =
        params
            .identity_pod
            .get(KEY_SIGNER)
            .ok_or(MainPodError::MissingField {
                pod_type: "Identity",
                field: "identity_server_pk",
            })?;

    // Start with the upvote verification predicate definitions and append REQUEST
    let mut query = get_upvote_verification_predicate();

    query.push_str(&format!(
        r#"

        REQUEST(
            upvote_verification({username}, {content_hash}, {identity_server_pk})
        )
        "#
    ));

    // Parse the complete query - only need upvote verification predicates
    let pod_params = PodNetProverSetup::get_params();
    let request = parse(&query, &pod_params, &[])
        .map_err(|e| MainPodError::ProofGeneration(format!("Parse error: {e:?}")))?
        .request_templates;

    // Provide both pods as facts
    let pods = [
        IndexablePod::signed_pod(params.identity_pod),
        IndexablePod::signed_pod(params.upvote_pod),
    ];

    // Let the solver find the proof
    let context = SolverContext::new(&pods, &[]);
    let (proof, _metrics) = solve(&request, &context, MetricsLevel::Counters)
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
        } else if params.upvote_pod.id() == pod_id {
            builder.add_signed_pod(params.upvote_pod);
        }
    }

    let main_pod = builder
        .prove(&*prover, &pod_params)
        .map_err(|e| MainPodError::ProofGeneration(format!("Prove error: {e:?}")))?;

    Ok(main_pod)
}

/// Verify an upvote verification MainPod using the pod2 solver
///
/// This verifies that the MainPod contains the expected public statements
/// and that the content hash and username match the expected values.
pub fn verify_upvote_verification_with_solver(
    main_pod: &MainPod,
    expected_username: &str,
    expected_content_hash: &Hash,
    expected_identity_server_pk: &Value,
) -> MainPodResult<()> {
    // Start with the upvote verification predicate definitions and append REQUEST
    let mut query = get_upvote_verification_predicate();

    let username_value = Value::from(expected_username);
    let content_hash_value = Value::from(*expected_content_hash);

    query.push_str(&format!(
        r#"

        REQUEST(
            upvote_verification({username_value}, {content_hash_value}, {expected_identity_server_pk})
        )
        "#
    ));

    // Parse the complete query - only need upvote verification predicates
    let pod_params = PodNetProverSetup::get_params();
    let request = parse(&query, &pod_params, &[])
        .map_err(|e| MainPodError::ProofGeneration(format!("Parse error: {e:?}")))?
        .request_templates;

    // Provide the MainPod as a fact
    let pods = [IndexablePod::main_pod(main_pod)];

    // Let the solver verify the proof
    let context = SolverContext::new(&pods, &[]);
    let (_proof, _metrics) = solve(&request, &context, MetricsLevel::Counters)
        .map_err(|e| MainPodError::ProofGeneration(format!("Solver error: {e:?}")))?;

    Ok(())
}

/// Parameters for upvote count base case proof generation
pub struct UpvoteCountBaseParams<'a> {
    pub content_hash: &'a Hash,
    pub use_mock_proofs: bool,
}

/// Generate an upvote count base case MainPod using the pod2 solver
///
/// This creates a MainPod that proves upvote_count(0, content_hash) using the base case
/// predicate: upvote_count_base(count, content_hash) where count = 0
pub fn prove_upvote_count_base_with_solver(
    params: UpvoteCountBaseParams,
) -> MainPodResult<MainPod> {
    use num_bigint::BigUint;
    use pod2::{
        backends::plonky2::{primitives::ec::schnorr::SecretKey, signedpod::Signer},
        frontend::SignedPodBuilder,
    };

    // Create a data pod with the content hash (signed by server)
    let pod_params = PodNetProverSetup::get_params();
    let mut data_builder = SignedPodBuilder::new(&pod_params);
    data_builder.insert("content_hash", *params.content_hash);

    // For now, use a dummy secret key for data pod signing
    // In practice, this should be signed by the server
    let dummy_sk = SecretKey(BigUint::from(12345u64));
    let signer = Signer(dummy_sk);
    let data_pod = data_builder
        .sign(&signer)
        .map_err(|e| MainPodError::ProofGeneration(format!("Failed to sign data pod: {e:?}")))?;

    // First parse the upvote verification predicate batch
    let upvote_verification_batch = parse(
        &crate::get_upvote_verification_predicate(),
        &pod_params,
        &[],
    )
    .map_err(|e| {
        MainPodError::ProofGeneration(format!("Parse error for upvote verification: {e:?}"))
    })?
    .custom_batch;

    // Then parse the upvote count predicate batch, providing the verification batch as a dependency
    let mut upvote_count_query = crate::get_upvote_count_predicate(upvote_verification_batch.id());

    let content_hash_value = Value::from(*params.content_hash);

    upvote_count_query.push_str(&format!(
        r#"

        REQUEST(
            upvote_count(0, {content_hash_value})
        )
        "#
    ));

    log::info!("Upvote count query: {upvote_count_query}");

    // Parse the complete query with the verification batch as a dependency
    let request = parse(
        &upvote_count_query,
        &pod_params,
        &[upvote_verification_batch],
    )
    .map_err(|e| MainPodError::ProofGeneration(format!("Parse error: {e:?}")))?
    .request_templates;

    // Provide the data pod as a fact
    let pods = [IndexablePod::signed_pod(&data_pod)];

    // Let the solver find the proof
    let context = SolverContext::new(&pods, &[]);
    let (proof, _metrics) = solve(&request, &context, MetricsLevel::Counters)
        .map_err(|e| MainPodError::ProofGeneration(format!("Solver error: {e:?}")))?;

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

    // Add the data pod that was referenced in the proof
    for pod_id in pod_ids {
        if data_pod.id() == pod_id {
            builder.add_signed_pod(&data_pod);
        }
    }

    let main_pod = builder
        .prove(&*prover, &pod_params)
        .map_err(|e| MainPodError::ProofGeneration(format!("Prove error: {e:?}")))?;

    Ok(main_pod)
}

/// Parameters for upvote count inductive case proof generation
pub struct UpvoteCountInductiveParams<'a> {
    pub content_hash: &'a Hash,
    pub previous_count: i64,
    pub previous_count_pod: &'a MainPod,
    pub upvote_verification_pod: &'a MainPod,
    pub use_mock_proofs: bool,
}

/// Generate an upvote count inductive case MainPod using the pod2 solver
///
/// This creates a MainPod that proves upvote_count(previous_count + 1, content_hash)
/// using the inductive case predicate
pub fn prove_upvote_count_inductive_with_solver(
    params: UpvoteCountInductiveParams,
) -> MainPodResult<MainPod> {
    // First parse the upvote verification predicate batch
    let pod_params = PodNetProverSetup::get_params();
    let upvote_verification_batch = parse(
        &crate::get_upvote_verification_predicate(),
        &pod_params,
        &[],
    )
    .map_err(|e| {
        MainPodError::ProofGeneration(format!("Parse error for upvote verification: {e:?}"))
    })?
    .custom_batch;

    // Then parse the upvote count predicate batch, providing the verification batch as a dependency
    let mut upvote_count_query = crate::get_upvote_count_predicate(upvote_verification_batch.id());

    let content_hash_value = Value::from(*params.content_hash);
    let new_count = params.previous_count + 1;

    upvote_count_query.push_str(&format!(
        r#"

        REQUEST(
            upvote_count({new_count}, {content_hash_value})
        )
        "#
    ));

    // Parse the complete query with the verification batch as a dependency
    let request = parse(
        &upvote_count_query,
        &pod_params,
        &[upvote_verification_batch],
    )
    .map_err(|e| MainPodError::ProofGeneration(format!("Parse error: {e:?}")))?
    .request_templates;

    // Provide both the previous count pod and upvote verification pod as facts
    let pods = [
        IndexablePod::main_pod(params.previous_count_pod),
        IndexablePod::main_pod(params.upvote_verification_pod),
    ];

    // Let the solver find the proof
    let context = SolverContext::new(&pods, &[]);
    let (proof, _metrics) = solve(&request, &context, MetricsLevel::Counters)
        .map_err(|e| MainPodError::ProofGeneration(format!("Solver error: {e:?}")))?;

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

    // Add the MainPods that were referenced in the proof
    for pod_id in pod_ids {
        if params.previous_count_pod.id() == pod_id {
            builder.add_recursive_pod(params.previous_count_pod.clone());
        } else if params.upvote_verification_pod.id() == pod_id {
            builder.add_recursive_pod(params.upvote_verification_pod.clone());
        }
    }

    let main_pod = builder
        .prove(&*prover, &pod_params)
        .map_err(|e| MainPodError::ProofGeneration(format!("Prove error: {e:?}")))?;

    Ok(main_pod)
}

/// Verify an upvote count MainPod using the pod2 solver
///
/// This verifies that the MainPod proves upvote_count(expected_count, expected_content_hash)
pub fn verify_upvote_count_with_solver(
    main_pod: &MainPod,
    expected_count: i64,
    expected_content_hash: &Hash,
) -> MainPodResult<()> {
    // First parse the upvote verification predicate batch
    let pod_params = PodNetProverSetup::get_params();
    let upvote_verification_batch = parse(
        &crate::get_upvote_verification_predicate(),
        &pod_params,
        &[],
    )
    .map_err(|e| {
        MainPodError::ProofGeneration(format!("Parse error for upvote verification: {e:?}"))
    })?
    .custom_batch;

    // Then parse the upvote count predicate batch, providing the verification batch as a dependency
    let mut upvote_count_query = crate::get_upvote_count_predicate(upvote_verification_batch.id());

    let content_hash_value = Value::from(*expected_content_hash);

    upvote_count_query.push_str(&format!(
        r#"

        REQUEST(
            upvote_count({expected_count}, {content_hash_value})
        )
        "#
    ));

    // Parse the complete query with the verification batch as a dependency
    let request = parse(
        &upvote_count_query,
        &pod_params,
        &[upvote_verification_batch],
    )
    .map_err(|e| MainPodError::ProofGeneration(format!("Parse error: {e:?}")))?
    .request_templates;

    // Provide the MainPod as a fact
    let pods = [IndexablePod::main_pod(main_pod)];

    // Let the solver verify the proof
    let context = SolverContext::new(&pods, &[]);
    let (_proof, _metrics) = solve(&request, &context, MetricsLevel::Counters)
        .map_err(|e| MainPodError::ProofGeneration(format!("Solver error: {e:?}")))?;

    Ok(())
}

#[cfg(test)]
mod tests {

    // Add unit tests for upvote verification functions
}
