//! Upvote count MainPod operations for recursive counting

use super::{MainPodError, MainPodResult, verify_mainpod_basics};
use crate::get_upvote_count_predicate;
use pod_utils::ValueExt;
use pod_utils::prover_setup::PodNetProverSetup;
use pod2::frontend::{MainPod, MainPodBuilder, SignedPod};
use pod2::lang::parse;
use pod2::middleware::{Hash, Statement, Value};
use pod2::op;

/// Parameters for upvote count base case proof generation
pub struct UpvoteCountBaseParams<'a> {
    pub data_pod: &'a SignedPod,
    pub content_hash: &'a Hash,
    pub use_mock_proofs: bool,
}

/// Parameters for upvote count inductive case proof generation
pub struct UpvoteCountInductiveParams<'a> {
    pub intermediate_main_pod: &'a MainPod,
    pub upvote_verification_main_pod: &'a MainPod,
    pub username: &'a str,
    pub content_hash: &'a Hash,
    pub identity_server_pk: &'a Value,
    pub expected_count: i64,
    pub use_mock_proofs: bool,
}

/// Generate a base case upvote count MainPod (count = 0)
///
/// This creates a MainPod that proves the base case for upvote counting,
/// establishing that a document has 0 upvotes initially.
pub fn prove_upvote_count_base(params: UpvoteCountBaseParams) -> MainPodResult<MainPod> {
    let pod_params = PodNetProverSetup::get_params();
    let (vd_set, prover) = PodNetProverSetup::create_prover_setup(params.use_mock_proofs)
        .map_err(MainPodError::ProofGeneration)?;

    // Parse predicates
    let predicate_input = get_upvote_count_predicate();
    let batch = parse(&predicate_input, &pod_params, &[])
        .map_err(|e| MainPodError::ProofGeneration(format!("Predicate parsing failed: {e}")))?
        .custom_batch;

    let upvote_count_base_pred = batch
        .predicate_ref_by_name("upvote_count_base")
        .ok_or_else(|| {
            MainPodError::ProofGeneration("Missing upvote_count_base predicate".to_string())
        })?;

    // Build base case main pod
    let mut builder = MainPodBuilder::new(&pod_params, vd_set);
    builder.add_signed_pod(params.data_pod);

    // Base case constraints
    let count_check = builder
        .priv_op(op!(eq, 0, 0))
        .map_err(|e| MainPodError::ProofGeneration(format!("Count check failed: {e}")))?;
    let content_hash_check = builder
        .priv_op(op!(
            eq,
            (params.data_pod, "content_hash"),
            *params.content_hash
        ))
        .map_err(|e| MainPodError::ProofGeneration(format!("Content hash check failed: {e}")))?;

    let _upvote_count_base = builder
        .pub_op(op!(
            custom,
            upvote_count_base_pred,
            count_check,
            content_hash_check
        ))
        .map_err(|e| MainPodError::ProofGeneration(format!("Base case statement failed: {e}")))?;

    let main_pod = builder.prove(prover.as_ref(), &pod_params).map_err(|e| {
        MainPodError::ProofGeneration(format!("Base case proof generation failed: {e}"))
    })?;

    main_pod.pod.verify().map_err(|e| {
        MainPodError::ProofGeneration(format!("Base case proof verification failed: {e}"))
    })?;

    Ok(main_pod)
}

/// Generate an inductive case upvote count MainPod (count = previous_count + 1)
///
/// This creates a MainPod that proves the inductive step for upvote counting,
/// showing that adding one valid upvote increases the count by 1.
pub fn prove_upvote_count_inductive(params: UpvoteCountInductiveParams) -> MainPodResult<MainPod> {
    let pod_params = PodNetProverSetup::get_params_with_batch_size(10); // Larger batch for inductive proofs
    let (vd_set, prover) = PodNetProverSetup::create_prover_setup(params.use_mock_proofs)
        .map_err(MainPodError::ProofGeneration)?;

    // Parse predicates
    let predicate_input = get_upvote_count_predicate();
    let batch = parse(&predicate_input, &pod_params, &[])
        .map_err(|e| MainPodError::ProofGeneration(format!("Predicate parsing failed: {e}")))?
        .custom_batch;

    let upvote_count_ind_pred =
        batch
            .predicate_ref_by_name("upvote_count_ind")
            .ok_or_else(|| {
                MainPodError::ProofGeneration("Missing upvote_count_ind predicate".to_string())
            })?;
    let upvote_count_pred = batch.predicate_ref_by_name("upvote_count").ok_or_else(|| {
        MainPodError::ProofGeneration("Missing upvote_count predicate".to_string())
    })?;
    let upvote_verification_pred = batch
        .predicate_ref_by_name("upvote_verification")
        .ok_or_else(|| {
            MainPodError::ProofGeneration("Missing upvote_verification predicate".to_string())
        })?;

    // Build inductive case main pod
    let mut builder = MainPodBuilder::new(&pod_params, vd_set);
    builder.add_recursive_pod(params.intermediate_main_pod.clone());
    builder.add_recursive_pod(params.upvote_verification_main_pod.clone());

    // Extract intermediate count from the previous proof
    let upvote_stmt = extract_upvote_pred_from_mainpod(params.intermediate_main_pod)?;
    let upvote_args = extract_upvote_args_from_statement(&upvote_stmt)?;
    let intermediate_count = upvote_args.first().and_then(Value::as_i64).ok_or_else(|| {
        MainPodError::Verification("Invalid upvote count in intermediate pod".to_string())
    })?;

    // Extract upvote verification from previous proof
    let upvote_verification_stmt = params
        .upvote_verification_main_pod
        .public_statements
        .iter()
        .find(
            |stmt| matches!(stmt, Statement::Custom(pred, _) if *pred == upvote_verification_pred),
        )
        .ok_or_else(|| {
            MainPodError::Verification("Missing upvote verification statement".to_string())
        })?;

    // Inductive case constraints
    let count_sum_check = builder
        .priv_op(op!(sum_of, params.expected_count, intermediate_count, 1))
        .map_err(|e| MainPodError::ProofGeneration(format!("Count sum check failed: {e}")))?;

    let upvote_count_inductive = builder
        .priv_op(op!(
            custom,
            upvote_count_ind_pred,
            upvote_stmt,
            count_sum_check,
            upvote_verification_stmt
        ))
        .map_err(|e| {
            MainPodError::ProofGeneration(format!("Inductive case statement failed: {e}"))
        })?;

    let _upvote_count = builder
        .pub_op(op!(
            custom,
            upvote_count_pred,
            Statement::None,
            upvote_count_inductive
        ))
        .map_err(|e| {
            MainPodError::ProofGeneration(format!("Predicate OR statement failed: {e}"))
        })?;

    let main_pod = builder.prove(prover.as_ref(), &pod_params).map_err(|e| {
        MainPodError::ProofGeneration(format!("Inductive case proof generation failed: {e}"))
    })?;

    main_pod.pod.verify().map_err(|e| {
        MainPodError::ProofGeneration(format!("Inductive case proof verification failed: {e}"))
    })?;

    Ok(main_pod)
}

/// Verify an upvote count MainPod
///
/// This verifies that the MainPod contains valid upvote count statements
/// and that the count matches the expected value.
pub fn verify_upvote_count(
    main_pod: &MainPod,
    expected_count: i64,
    expected_content_hash: &Hash,
) -> MainPodResult<()> {
    // Verify basic MainPod structure
    verify_mainpod_basics(main_pod)?;

    // Extract arguments with the macro
    let (count, content_hash) = crate::extract_mainpod_args!(
        main_pod,
        get_upvote_count_predicate(),
        "upvote_count",
        count: as_i64,
        content_hash: as_hash
    )?;

    // Verify extracted data matches expected values
    if count != expected_count {
        return Err(MainPodError::InvalidValue {
            field: "count",
            expected: expected_count.to_string(),
        });
    }

    if content_hash != *expected_content_hash {
        return Err(MainPodError::InvalidValue {
            field: "content_hash",
            expected: "matching content hash".to_string(),
        });
    }

    Ok(())
}

/// Extract upvote count from a MainPod's public statements
///
/// Helper function to extract the count value from an upvote count MainPod
fn extract_upvote_pred_from_mainpod(main_pod: &MainPod) -> MainPodResult<Statement> {
    let params = PodNetProverSetup::get_params();
    let predicate_input = get_upvote_count_predicate();
    let batch = parse(&predicate_input, &params, &[])
        .map_err(|e| MainPodError::Verification(format!("Predicate parsing failed: {e}")))?
        .custom_batch;

    let upvote_count_pred = batch
        .predicate_ref_by_name("upvote_count")
        .ok_or_else(|| MainPodError::Verification("Missing upvote_count predicate".to_string()))?;

    // Find the upvote count statement
    let pred = main_pod
        .public_statements
        .iter()
        .find(|stmt| matches!(stmt, Statement::Custom(pred, _) if *pred == upvote_count_pred))
        .ok_or_else(|| {
            MainPodError::Verification("MainPod missing upvote_count statement".to_string())
        })?;

    Ok(pred.clone())
}

fn extract_upvote_args_from_statement(statement: &Statement) -> MainPodResult<Vec<Value>> {
    let params = PodNetProverSetup::get_params();
    let predicate_input = get_upvote_count_predicate();
    let batch = parse(&predicate_input, &params, &[])
        .map_err(|e| MainPodError::Verification(format!("Predicate parsing failed: {e}")))?
        .custom_batch;

    let upvote_count_pred = batch
        .predicate_ref_by_name("upvote_count")
        .ok_or_else(|| MainPodError::Verification("Missing upvote_count predicate".to_string()))?;

    if let Statement::Custom(pred, args) = statement {
        if *pred == upvote_count_pred {
            return Ok(args.clone());
        }
    }
    Err(MainPodError::Verification(
        "Statement is not an upvote_count predicate".to_string(),
    ))
}

#[cfg(test)]
mod tests {

    // Add unit tests for upvote count functions
}
