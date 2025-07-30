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

/// Generate a publish verification MainPod
///
/// This creates a MainPod that cryptographically proves:
/// - Identity pod was signed by a registered identity server
/// - Document pod was signed by the user from the identity pod
/// - Cross-verification between identity and document signers
/// - Content hash verification
pub fn prove_publish_verification(_params: PublishProofParams) -> MainPodResult<MainPod> {
    //    // Extract required values from pods
    //    let username = extract_username(params.identity_pod)?;
    //
    //    // Extract values from document pods
    //    let post_id = params
    //        .document1_pod
    //        .get("post_id")
    //        .ok_or(MainPodError::MissingField {
    //            pod_type: "Document1",
    //            field: "post_id",
    //        })?
    //        .clone();
    //    let tags = params
    //        .document1_pod
    //        .get("tags")
    //        .ok_or(MainPodError::MissingField {
    //            pod_type: "Document1",
    //            field: "tags",
    //        })?
    //        .clone();
    //    let authors = params
    //        .document2_pod
    //        .get("authors")
    //        .ok_or(MainPodError::MissingField {
    //            pod_type: "Document2",
    //            field: "authors",
    //        })?
    //        .clone();
    //    let reply_to = params
    //        .document2_pod
    //        .get("reply_to")
    //        .ok_or(MainPodError::MissingField {
    //            pod_type: "Document2",
    //            field: "reply_to",
    //        })?
    //        .clone();
    //    let uploader = params
    //        .document2_pod
    //        .get("uploader_id")
    //        .ok_or(MainPodError::MissingField {
    //            pod_type: "Document2",
    //            field: "uploader_id",
    //        })?
    //        .clone();
    //
    //    // Step 1: Create individual proofs using the unified macro syntax
    //    let identity_main_pod = main_pod!(
    //        params.use_mock_proofs,
    //        get_publish_verification_predicate,
    //        using [params.identity_pod], {
    //            identity_verified(params.identity_pod, username) => {
    //                eq((params.identity_pod, KEY_TYPE), PodType::Signed),
    //                eq((params.identity_pod, "username"), username),
    //            }
    //        }
    //    )?;
    //
    //    let document1_main_pod = main_pod!(
    //        params.use_mock_proofs,
    //        get_publish_verification_predicate,
    //        using [params.document1_pod], {
    //            document_verified1(params.document1_pod, params.content_hash, post_id, tags) => {
    //                eq((params.document1_pod, "request_type"), "publish"),
    //                eq((params.document1_pod, "content_hash"), *params.content_hash),
    //                eq((params.document1_pod, "post_id"), post_id.clone()),
    //                eq((params.document1_pod, "tags"), tags.clone()),
    //            }
    //        }
    //    )?;
    //
    //    let document2_main_pod = main_pod!(
    //        params.use_mock_proofs,
    //        get_publish_verification_predicate,
    //        using [params.document2_pod], {
    //            document_verified2(params.document2_pod, authors, reply_to, uploader) => {
    //                eq((params.document2_pod, "authors"), authors.clone()),
    //                eq((params.document2_pod, "reply_to"), reply_to.clone()),
    //                eq((params.document2_pod, "uploader_id"), uploader.clone()),
    //            }
    //        }
    //    )?;
    //
    //    // Step 2: Get the public statements before the pods are moved
    //    let identity_statement = identity_main_pod.pod.pub_statements()[1].clone();
    //    let document1_statement = document1_main_pod.pod.pub_statements()[1].clone();
    //    let document2_statement = document2_main_pod.pod.pub_statements()[1].clone();
    //
    //    // Create intermediate pod to work around max recursive pod number...
    //    let intermediate_pod = main_pod!(
    //        params.use_mock_proofs,
    //        get_publish_verification_predicate,
    //        using [params.identity_pod, params.document1_pod, params.document2_pod]
    //        with recursive [identity_main_pod.clone(), document1_main_pod.clone()], {
    //            identity_verified(params.identity_pod, username) => (identity_statement.clone()),
    //            document_verified1(params.document1_pod, params.content_hash, post_id, tags) => (document1_statement.clone()),
    //        }
    //    )?;
    //
    //    // Step 3: Create final proof using recursive statements
    //    let final_main_pod = main_pod!(
    //        params.use_mock_proofs,
    //        get_publish_verification_predicate,
    //        using [params.identity_pod, params.document1_pod, params.document2_pod]
    //        with recursive [intermediate_pod, document2_main_pod], {
    //            identity_verified(params.identity_pod, username) => identity_statement,
    //            document_verified1(params.document1_pod, params.content_hash, post_id, tags) => document1_statement,
    //            document_verified2(params.document2_pod, authors, reply_to, uploader) => document2_statement,
    //        }
    //    )?;
    //
    //    Ok(final_main_pod)
    unimplemented!();
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
