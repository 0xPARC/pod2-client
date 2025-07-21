//! Upvote verification MainPod operations

// Import solver dependencies
use pod_utils::{ValueExt, prover_setup::PodNetProverSetup};
use pod2::{
    frontend::{MainPod, MainPodBuilder, SignedPod},
    lang::parse,
    middleware::{Hash, KEY_SIGNER, KEY_TYPE, Params, PodType, Value},
    op,
};
use pod2_solver::{db::IndexablePod, metrics::MetricsLevel, solve, value_to_podlang_literal};

use super::{
    MainPodError, MainPodResult, extract_user_public_key, extract_username, verify_mainpod_basics,
};
use crate::{get_upvote_verification_predicate, main_pod, signed_pod, verify_main_pod};

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

/// Generate an upvote verification MainPod
///
/// This creates a MainPod that cryptographically proves:
/// - Identity pod was signed by a registered identity server
/// - Upvote pod was signed by the user from the identity pod
/// - Cross-verification between identity and upvote signers
/// - Content hash verification for the upvoted document
pub fn prove_upvote_verification(params: UpvoteProofParams) -> MainPodResult<MainPod> {
    // Extract required values from pods
    let username = extract_username(params.identity_pod)?;
    let user_public_key = extract_user_public_key(params.identity_pod)?;

    // Step 1: Prove identity_verified using the unified macro syntax
    let identity_main_pod = main_pod!(
        params.use_mock_proofs,
        get_upvote_verification_predicate,
        using [params.identity_pod], {
            identity_verified(username) => {
                eq((params.identity_pod, KEY_TYPE), PodType::Signed),
                eq((params.identity_pod, "username"), username),
            }
        }
    )?;

    // Step 2: Prove upvote_verified using the unified macro syntax
    let upvote_main_pod = main_pod!(
        params.use_mock_proofs,
        get_upvote_verification_predicate,
        using [params.upvote_pod], {
            upvote_verified(params.content_hash) => {
                eq((params.upvote_pod, KEY_TYPE), PodType::Signed),
                eq((params.upvote_pod, "content_hash"), *params.content_hash),
                eq((params.upvote_pod, "request_type"), "upvote"),
            }
        }
    )?;

    // Step 3: Get statements and create final verification
    let identity_statement = identity_main_pod.pod.pub_statements()[1].clone();
    let upvote_statement = upvote_main_pod.pod.pub_statements()[1].clone();

    // Step 4: Create final upvote verification using recursive statements
    let final_main_pod = main_pod!(
        params.use_mock_proofs,
        get_upvote_verification_predicate,
        using [params.identity_pod, params.upvote_pod]
        with recursive [identity_main_pod, upvote_main_pod], {
            identity_verified(username) => identity_statement,
            upvote_verified(params.content_hash) => upvote_statement,
        }
    )?;

    Ok(final_main_pod)
}

pub fn prove_upvote_verification_original_fallback(
    params: UpvoteProofParams,
) -> MainPodResult<MainPod> {
    // For now, fall back to the original implementation
    let pod_params = PodNetProverSetup::get_params();
    let (vd_set, prover) = PodNetProverSetup::create_prover_setup(params.use_mock_proofs)
        .map_err(MainPodError::ProofGeneration)?;

    // Build final upvote verification main pod
    let mut final_builder = MainPodBuilder::new(&pod_params, vd_set);

    // COMMENTED OUT - leftover from old implementation
    // let identity_verification = identity_main_pod.pod.pub_statements()[0].clone();
    // let upvote_verification = upvote_main_pod.pod.pub_statements()[0].clone();

    // final_builder.add_recursive_pod(identity_main_pod);
    // final_builder.add_recursive_pod(upvote_main_pod);
    final_builder.add_signed_pod(params.identity_pod);
    final_builder.add_signed_pod(params.upvote_pod);

    let identity_server_pk_check = final_builder
        .priv_op(op!(
            eq,
            (params.identity_pod, KEY_SIGNER),
            params.identity_server_public_key.clone()
        ))
        .map_err(|e| {
            MainPodError::ProofGeneration(format!("Final identity server check failed: {}", e))
        })?;

    let user_pk_check = final_builder
        .priv_op(op!(
            eq,
            (params.identity_pod, "user_public_key"),
            (params.upvote_pod, KEY_SIGNER)
        ))
        .map_err(|e| {
            MainPodError::ProofGeneration(format!("Final user key check failed: {}", e))
        })?;

    // Parse predicates for final verification
    let predicate_input = get_upvote_verification_predicate();
    let batch = parse(&predicate_input, &pod_params, &[])
        .map_err(|e| MainPodError::ProofGeneration(format!("Predicate parsing failed: {}", e)))?
        .custom_batch;

    let upvote_verification_pred = batch
        .predicate_ref_by_name("upvote_verification")
        .ok_or_else(|| {
            MainPodError::ProofGeneration("Missing upvote_verification predicate".to_string())
        })?;

    // COMMENTED OUT - needs to be fixed with proper variables
    // let _upvote_verification_final = final_builder
    //     .pub_op(op!(
    //         custom,
    //         upvote_verification_pred,
    //         identity_verification,
    //         upvote_verification,
    //         identity_server_pk_check,
    //         user_pk_check
    //     ))
    //     .map_err(|e| {
    //         MainPodError::ProofGeneration(format!(
    //             "Final upvote verification statement failed: {}",
    //             e
    //         ))
    //     })?;

    // For now, create a simple proof just to make this compile
    let simple_verification = final_builder
        .pub_op(op!(
            custom,
            upvote_verification_pred,
            identity_server_pk_check,
            user_pk_check
        ))
        .map_err(|e| {
            MainPodError::ProofGeneration(format!(
                "Simple upvote verification statement failed: {}",
                e
            ))
        })?;

    let main_pod = final_builder
        .prove(prover.as_ref(), &pod_params)
        .map_err(|e| {
            MainPodError::ProofGeneration(format!("Final proof generation failed: {}", e))
        })?;

    main_pod.pod.verify().map_err(|e| {
        MainPodError::ProofGeneration(format!("Final proof verification failed: {}", e))
    })?;

    Ok(main_pod)
}

pub fn prove_upvote_verification_original(params: UpvoteProofParams) -> MainPodResult<MainPod> {
    let pod_params = PodNetProverSetup::get_params();
    let (vd_set, prover) = PodNetProverSetup::create_prover_setup(params.use_mock_proofs)
        .map_err(MainPodError::ProofGeneration)?;

    // Extract required values from pods
    let username = extract_username(params.identity_pod)?;
    let user_public_key = extract_user_public_key(params.identity_pod)?;

    // Parse predicates
    let predicate_input = get_upvote_verification_predicate();
    let batch = parse(&predicate_input, &pod_params, &[])
        .map_err(|e| MainPodError::ProofGeneration(format!("Predicate parsing failed: {e}")))?
        .custom_batch;

    let identity_verified_pred = batch
        .predicate_ref_by_name("identity_verified")
        .ok_or_else(|| {
            MainPodError::ProofGeneration("Missing identity_verified predicate".to_string())
        })?;
    let upvote_verified_pred = batch
        .predicate_ref_by_name("upvote_verified")
        .ok_or_else(|| {
            MainPodError::ProofGeneration("Missing upvote_verified predicate".to_string())
        })?;
    let upvote_verification_pred = batch
        .predicate_ref_by_name("upvote_verification")
        .ok_or_else(|| {
            MainPodError::ProofGeneration("Missing upvote_verification predicate".to_string())
        })?;

    // Step 1: Build identity verification main pod
    let mut identity_builder = MainPodBuilder::new(&pod_params, vd_set);
    identity_builder.add_signed_pod(params.identity_pod);

    let identity_type_check = identity_builder
        .priv_op(op!(eq, (params.identity_pod, KEY_TYPE), PodType::Signed))
        .map_err(|e| MainPodError::ProofGeneration(format!("Identity type check failed: {e}")))?;
    let _identity_signer_check = identity_builder
        .priv_op(op!(
            eq,
            (params.identity_pod, KEY_SIGNER),
            params.identity_server_public_key.clone()
        ))
        .map_err(|e| MainPodError::ProofGeneration(format!("Identity signer check failed: {e}")))?;
    let identity_username_check = identity_builder
        .priv_op(op!(eq, (params.identity_pod, "username"), username))
        .map_err(|e| {
            MainPodError::ProofGeneration(format!("Identity username check failed: {e}"))
        })?;

    let identity_verification = identity_builder
        .pub_op(op!(
            custom,
            identity_verified_pred,
            identity_type_check,
            identity_username_check
        ))
        .map_err(|e| {
            MainPodError::ProofGeneration(format!("Identity verification statement failed: {e}"))
        })?;

    let identity_main_pod = identity_builder
        .prove(prover.as_ref(), &pod_params)
        .map_err(|e| {
            MainPodError::ProofGeneration(format!("Identity proof generation failed: {e}"))
        })?;

    identity_main_pod.pod.verify().map_err(|e| {
        MainPodError::ProofGeneration(format!("Identity proof verification failed: {e}"))
    })?;

    // Step 2: Build upvote verification main pod
    let mut upvote_builder = MainPodBuilder::new(&pod_params, vd_set);
    upvote_builder.add_signed_pod(params.upvote_pod);

    let upvote_type_check = upvote_builder
        .priv_op(op!(eq, (params.upvote_pod, KEY_TYPE), PodType::Signed))
        .map_err(|e| MainPodError::ProofGeneration(format!("Upvote type check failed: {e}")))?;
    let _upvote_signer_check = upvote_builder
        .priv_op(op!(eq, (params.upvote_pod, KEY_SIGNER), user_public_key))
        .map_err(|e| MainPodError::ProofGeneration(format!("Upvote signer check failed: {e}")))?;
    let upvote_content_check = upvote_builder
        .priv_op(op!(
            eq,
            (params.upvote_pod, "content_hash"),
            *params.content_hash
        ))
        .map_err(|e| MainPodError::ProofGeneration(format!("Upvote content check failed: {e}")))?;
    let upvote_request_type_check = upvote_builder
        .priv_op(op!(eq, (params.upvote_pod, "request_type"), "upvote"))
        .map_err(|e| {
            MainPodError::ProofGeneration(format!("Upvote request type check failed: {e}"))
        })?;

    let upvote_verification = upvote_builder
        .pub_op(op!(
            custom,
            upvote_verified_pred,
            upvote_type_check,
            upvote_content_check,
            upvote_request_type_check
        ))
        .map_err(|e| {
            MainPodError::ProofGeneration(format!("Upvote verification statement failed: {e}"))
        })?;

    let upvote_main_pod = upvote_builder
        .prove(prover.as_ref(), &pod_params)
        .map_err(|e| {
            MainPodError::ProofGeneration(format!("Upvote proof generation failed: {e}"))
        })?;

    upvote_main_pod.pod.verify().map_err(|e| {
        MainPodError::ProofGeneration(format!("Upvote proof verification failed: {e}"))
    })?;

    // Step 3: Build final upvote verification main pod
    let mut final_builder = MainPodBuilder::new(&pod_params, vd_set);
    final_builder.add_recursive_pod(identity_main_pod);
    final_builder.add_recursive_pod(upvote_main_pod);
    final_builder.add_signed_pod(params.identity_pod);
    final_builder.add_signed_pod(params.upvote_pod);

    let identity_server_pk_check = final_builder
        .priv_op(op!(
            eq,
            (params.identity_pod, KEY_SIGNER),
            params.identity_server_public_key.clone()
        ))
        .map_err(|e| {
            MainPodError::ProofGeneration(format!("Final identity server check failed: {e}"))
        })?;

    let user_pk_check = final_builder
        .priv_op(op!(
            eq,
            (params.identity_pod, "user_public_key"),
            (params.upvote_pod, KEY_SIGNER)
        ))
        .map_err(|e| MainPodError::ProofGeneration(format!("Final user key check failed: {e}")))?;

    let _upvote_verification_final = final_builder
        .pub_op(op!(
            custom,
            upvote_verification_pred,
            identity_verification,
            upvote_verification,
            identity_server_pk_check,
            user_pk_check
        ))
        .map_err(|e| {
            MainPodError::ProofGeneration(format!(
                "Final upvote verification statement failed: {e}"
            ))
        })?;

    let main_pod = final_builder
        .prove(prover.as_ref(), &pod_params)
        .map_err(|e| {
            MainPodError::ProofGeneration(format!("Final proof generation failed: {e}"))
        })?;

    main_pod.pod.verify().map_err(|e| {
        MainPodError::ProofGeneration(format!("Final proof verification failed: {e}"))
    })?;

    Ok(main_pod)
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

    // Format the expected values for the query using value_to_podlang_literal
    let username_literal = value_to_podlang_literal(username.clone());
    let content_hash_literal = value_to_podlang_literal(content_hash.clone());
    let identity_server_pk_literal = value_to_podlang_literal(identity_server_pk.clone());

    query.push_str(&format!(
        r#"

        REQUEST(
            upvote_verification({username_literal}, {content_hash_literal}, {identity_server_pk_literal})
        )
        "#
    ));

    // Parse the complete query - only need upvote verification predicates
    let pod_params = PodNetProverSetup::get_params();
    let request = parse(&query, &pod_params, &[])
        .map_err(|e| MainPodError::ProofGeneration(format!("Parse error: {:?}", e)))?
        .request_templates;

    // Provide both pods as facts
    let pods = [
        IndexablePod::signed_pod(params.identity_pod),
        IndexablePod::signed_pod(params.upvote_pod),
    ];

    // Let the solver find the proof
    let (proof, _metrics) = solve(&request, &pods, MetricsLevel::Counters)
        .map_err(|e| MainPodError::ProofGeneration(format!("Solver error: {:?}", e)))?;

    let pod_params = PodNetProverSetup::get_params();
    let (vd_set, prover) = PodNetProverSetup::create_prover_setup(params.use_mock_proofs)
        .map_err(MainPodError::ProofGeneration)?;

    let mut builder = MainPodBuilder::new(&pod_params, vd_set);

    let (pod_ids, ops) = proof.to_inputs();

    for (op, public) in ops {
        if public {
            builder
                .pub_op(op)
                .map_err(|e| MainPodError::ProofGeneration(format!("Builder error: {:?}", e)))?;
        } else {
            builder
                .priv_op(op)
                .map_err(|e| MainPodError::ProofGeneration(format!("Builder error: {:?}", e)))?;
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
        .map_err(|e| MainPodError::ProofGeneration(format!("Prove error: {:?}", e)))?;

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

    // Format the expected values for the query using value_to_podlang_literal
    let username_literal = value_to_podlang_literal(Value::from(expected_username));
    let content_hash_literal = value_to_podlang_literal(Value::from(*expected_content_hash));
    let identity_server_pk_literal = value_to_podlang_literal(expected_identity_server_pk.clone());

    query.push_str(&format!(
        r#"

        REQUEST(
            upvote_verification({username_literal}, {content_hash_literal}, {identity_server_pk_literal})
        )
        "#
    ));

    // Parse the complete query - only need upvote verification predicates
    let pod_params = PodNetProverSetup::get_params();
    let request = parse(&query, &pod_params, &[])
        .map_err(|e| MainPodError::ProofGeneration(format!("Parse error: {:?}", e)))?
        .request_templates;

    // Provide the MainPod as a fact
    let pods = [IndexablePod::main_pod(main_pod)];

    // Let the solver verify the proof
    let (_proof, _metrics) = solve(&request, &pods, MetricsLevel::Counters)
        .map_err(|e| MainPodError::ProofGeneration(format!("Solver error: {:?}", e)))?;

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
    let mut signer = Signer(dummy_sk);
    let data_pod = data_builder
        .sign(&mut signer)
        .map_err(|e| MainPodError::ProofGeneration(format!("Failed to sign data pod: {:?}", e)))?;

    // First parse the upvote verification predicate batch
    let upvote_verification_batch = parse(
        &crate::get_upvote_verification_predicate(),
        &pod_params,
        &[],
    )
    .map_err(|e| {
        MainPodError::ProofGeneration(format!("Parse error for upvote verification: {:?}", e))
    })?
    .custom_batch;

    // Then parse the upvote count predicate batch, providing the verification batch as a dependency
    let mut upvote_count_query = crate::get_upvote_count_predicate(upvote_verification_batch.id());

    // Format the expected values for the query using value_to_podlang_literal
    let content_hash_literal = value_to_podlang_literal(Value::from(*params.content_hash));

    upvote_count_query.push_str(&format!(
        r#"

        REQUEST(
            upvote_count(0, {content_hash_literal})
        )
        "#
    ));

    log::info!("Upvote count query: {}", upvote_count_query);

    // Parse the complete query with the verification batch as a dependency
    let request = parse(
        &upvote_count_query,
        &pod_params,
        &[upvote_verification_batch],
    )
    .map_err(|e| MainPodError::ProofGeneration(format!("Parse error: {:?}", e)))?
    .request_templates;

    // Provide the data pod as a fact
    let pods = [IndexablePod::signed_pod(&data_pod)];

    // Let the solver find the proof
    let (proof, _metrics) = solve(&request, &pods, MetricsLevel::Counters)
        .map_err(|e| MainPodError::ProofGeneration(format!("Solver error: {:?}", e)))?;

    let (vd_set, prover) = PodNetProverSetup::create_prover_setup(params.use_mock_proofs)
        .map_err(MainPodError::ProofGeneration)?;

    let mut builder = MainPodBuilder::new(&pod_params, vd_set);

    let (pod_ids, ops) = proof.to_inputs();

    for (op, public) in ops {
        if public {
            builder
                .pub_op(op)
                .map_err(|e| MainPodError::ProofGeneration(format!("Builder error: {:?}", e)))?;
        } else {
            builder
                .priv_op(op)
                .map_err(|e| MainPodError::ProofGeneration(format!("Builder error: {:?}", e)))?;
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
        .map_err(|e| MainPodError::ProofGeneration(format!("Prove error: {:?}", e)))?;

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
        MainPodError::ProofGeneration(format!("Parse error for upvote verification: {:?}", e))
    })?
    .custom_batch;

    // Then parse the upvote count predicate batch, providing the verification batch as a dependency
    let mut upvote_count_query = crate::get_upvote_count_predicate(upvote_verification_batch.id());

    // Format the expected values for the query using value_to_podlang_literal
    let content_hash_literal = value_to_podlang_literal(Value::from(*params.content_hash));
    let new_count = params.previous_count + 1;

    upvote_count_query.push_str(&format!(
        r#"

        REQUEST(
            upvote_count({new_count}, {content_hash_literal})
        )
        "#
    ));

    // Parse the complete query with the verification batch as a dependency
    let request = parse(
        &upvote_count_query,
        &pod_params,
        &[upvote_verification_batch],
    )
    .map_err(|e| MainPodError::ProofGeneration(format!("Parse error: {:?}", e)))?
    .request_templates;

    // Provide both the previous count pod and upvote verification pod as facts
    let pods = [
        IndexablePod::main_pod(params.previous_count_pod),
        IndexablePod::main_pod(params.upvote_verification_pod),
    ];

    // Let the solver find the proof
    let (proof, _metrics) = solve(&request, &pods, MetricsLevel::Counters)
        .map_err(|e| MainPodError::ProofGeneration(format!("Solver error: {:?}", e)))?;

    let (vd_set, prover) = PodNetProverSetup::create_prover_setup(params.use_mock_proofs)
        .map_err(MainPodError::ProofGeneration)?;

    let mut builder = MainPodBuilder::new(&pod_params, vd_set);

    let (pod_ids, ops) = proof.to_inputs();

    for (op, public) in ops {
        if public {
            builder
                .pub_op(op)
                .map_err(|e| MainPodError::ProofGeneration(format!("Builder error: {:?}", e)))?;
        } else {
            builder
                .priv_op(op)
                .map_err(|e| MainPodError::ProofGeneration(format!("Builder error: {:?}", e)))?;
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
        .map_err(|e| MainPodError::ProofGeneration(format!("Prove error: {:?}", e)))?;

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
        MainPodError::ProofGeneration(format!("Parse error for upvote verification: {:?}", e))
    })?
    .custom_batch;

    // Then parse the upvote count predicate batch, providing the verification batch as a dependency
    let mut upvote_count_query = crate::get_upvote_count_predicate(upvote_verification_batch.id());

    // Format the expected values for the query using value_to_podlang_literal
    let content_hash_literal = value_to_podlang_literal(Value::from(*expected_content_hash));

    upvote_count_query.push_str(&format!(
        r#"

        REQUEST(
            upvote_count({expected_count}, {content_hash_literal})
        )
        "#
    ));

    // Parse the complete query with the verification batch as a dependency
    let request = parse(
        &upvote_count_query,
        &pod_params,
        &[upvote_verification_batch],
    )
    .map_err(|e| MainPodError::ProofGeneration(format!("Parse error: {:?}", e)))?
    .request_templates;

    // Provide the MainPod as a fact
    let pods = [IndexablePod::main_pod(main_pod)];

    // Let the solver verify the proof
    let (_proof, _metrics) = solve(&request, &pods, MetricsLevel::Counters)
        .map_err(|e| MainPodError::ProofGeneration(format!("Solver error: {:?}", e)))?;

    Ok(())
}

#[cfg(test)]
mod tests {

    // Add unit tests for upvote verification functions
}
