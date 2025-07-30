//! Publish verification MainPod operations

use pod_utils::prover_setup::PodNetProverSetup;
use pod2::{
    frontend::{MainPod, MainPodBuilder, SignedPod},
    lang::parse,
    middleware::{KEY_SIGNER, Params, Value, containers::Dictionary},
};
// Import solver dependencies
use pod2_solver::{SolverContext, db::IndexablePod, metrics::MetricsLevel, solve};

use super::{MainPodError, MainPodResult};
use crate::get_publish_verification_predicate;
// Import the main_pod macro

/// Parameters for publish verification proof generation
pub struct PublishProofParams<'a> {
    pub identity_pod: &'a SignedPod,
    pub document_pod: &'a SignedPod,
    pub use_mock_proofs: bool,
}

/// Generate a publish verification MainPod using the pod2 solver
///
/// This creates a MainPod that cryptographically proves the same properties as
/// prove_publish_verification but uses the automated solver approach instead
/// of manual proof construction.
pub fn prove_publish_verification_with_solver(
    params: PublishProofParams,
) -> MainPodResult<MainPod> {
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

    // Start with the existing predicate definitions and append REQUEST
    let mut query = get_publish_verification_predicate();

    query.push_str(&format!(
        r#"

        REQUEST(
            publish_verified({username}, {data}, {identity_server_pk})
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
        .prove(&*prover)
        .map_err(|e| MainPodError::ProofGeneration(format!("Prove error: {e:?}")))?;

    Ok(main_pod)
}

pub fn verify_publish_verification_with_solver(
    main_pod: &MainPod,
    expected_username: &str,
    expected_data: &Dictionary,
    expected_identity_server_pk: &Value,
) -> MainPodResult<()> {
    // Start with the existing predicate definitions and append REQUEST
    let mut query = get_publish_verification_predicate();

    let username_value = Value::from(expected_username);
    let data_value = Value::from(expected_data.clone());

    query.push_str(&format!(
        r#"

        REQUEST(
            publish_verified({username_value}, {data_value}, {expected_identity_server_pk})
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
    let pods = [IndexablePod::main_pod(main_pod)];

    // Let the solver find the proof
    let context = SolverContext::new(&pods, &[]);
    let (proof, _metrics) = solve(request.templates(), &context, MetricsLevel::Counters)
        .map_err(|e| MainPodError::ProofGeneration(format!("Solver error: {e:?}")))?;
    println!("GOT PROOF: {proof}");

    Ok(())
}

#[cfg(test)]
mod tests {

    // Add unit tests for publish verification functions
}
