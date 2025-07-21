/// A macro for creating SignedPods with reduced boilerplate.
///
/// This macro simplifies the creation of signed pods by automatically handling
/// the builder pattern and signing process.
///
/// # Syntax
/// ```rust
/// signed_pod!(pod_params, secret_key, {
///     "field1" => value1,
///     "field2" => value2,
///     // ... more fields
/// })
/// ```
///
/// # Arguments
/// * `pod_params` - The pod parameters from `PodNetProverSetup::get_params()`
/// * `secret_key` - The secret key for signing (will be cloned automatically)
/// * Fields - Key-value pairs where keys are strings and values are POD-compatible types
///
/// # Returns
/// Returns a `Result<SignedPod, Error>` that must be handled with `?` or `.unwrap()`.
///
/// # Example
/// ```rust
/// let params = PodNetProverSetup::get_params();
/// let secret_key = /* your secret key */;
///
/// let pod = signed_pod!(&params, secret_key.clone(), {
///     "request_type" => "publish",
///     "content_hash" => content_hash,
///     "post_id" => post_id_num.unwrap_or(-1),
///     "tags" => tag_set,
/// })?;
/// ```
#[macro_export]
macro_rules! signed_pod {
    ($params:expr, $secret_key:expr, {
        $($key:expr => $value:expr),* $(,)?
    }) => {{
        let mut builder = SignedPodBuilder::new($params);
        $(
            builder.insert($key, $value);
        )*
        builder.sign(&mut Signer($secret_key))?
    }};
}

/// A unified macro for creating MainPods with both inline operations and recursive pod references.
///
/// This macro provides a clean, consistent syntax for creating cryptographic proofs using MainPods.
/// It supports both inline operations (like `eq`, `custom`) and references to statements from
/// previously created MainPods.
///
/// # Syntax
///
/// ## With Recursive Pods
/// ```rust
/// main_pod!(
///     use_mock_proofs,
///     predicate_function,
///     using [pod1, pod2, ...]
///     with recursive [recursive_pod1, recursive_pod2], {
///         predicate_name(args) => statement_from_recursive_pod,
///         other_predicate(args) => {
///             eq((pod, "field"), value),
///             eq((pod, "other_field"), other_value),
///         },
///     }
/// )
/// ```
///
/// ## Without Recursive Pods
/// ```rust
/// main_pod!(
///     use_mock_proofs,
///     predicate_function,
///     using [pod1, pod2, ...], {
///         predicate_name(args) => {
///             eq((pod, "field"), value),
///             eq((pod, "other_field"), other_value),
///         },
///     }
/// )
/// ```
///
/// # Arguments
///
/// * `use_mock_proofs` - Boolean indicating whether to use mock proofs (faster) or real ZK proofs
/// * `predicate_function` - Function that returns the predicate string (e.g., `get_publish_verification_predicate`)
/// * `using [...]` - Array of SignedPods that will be added to the MainPod builder
/// * `with recursive [...]` - Optional array of MainPods whose statements can be referenced
/// * Predicate mappings - Each predicate maps to either:
///   - A statement from a recursive pod: `predicate(args) => statement`
///   - Inline operations: `predicate(args) => { op1(...), op2(...), ... }`
///
/// # Returns
/// Returns a `Result<MainPod, MainPodError>` that must be handled with `?` or `.unwrap()`.
///
/// # Examples
///
/// ## Basic Usage with Inline Operations
/// ```rust
/// let main_pod = main_pod!(
///     params.use_mock_proofs,
///     get_publish_verification_predicate,
///     using [params.identity_pod], {
///         identity_verified(params.identity_pod, username) => {
///             eq((params.identity_pod, KEY_TYPE), PodType::Signed),
///             eq((params.identity_pod, "username"), username),
///         }
///     }
/// )?;
/// ```
///
/// ## Advanced Usage with Recursive Pods
/// ```rust
/// // First create individual proofs
/// let identity_main_pod = main_pod!(/* ... */)?;
/// let document_main_pod = main_pod!(/* ... */)?;
///
/// // Extract statements - NOTE: Parentheses are required around expressions!
/// let identity_statement = identity_main_pod.pod.pub_statements()[1].clone();
/// let document_statement = document_main_pod.pod.pub_statements()[1].clone();
///
/// // Combine them in a final proof
/// let final_main_pod = main_pod!(
///     params.use_mock_proofs,
///     get_publish_verification_predicate,
///     using [params.identity_pod, params.document_pod]
///     with recursive [identity_main_pod, document_main_pod], {
///         identity_verified(params.identity_pod, username) => (identity_statement.clone()),
///         document_verified(params.document_pod, content_hash) => (document_statement.clone()),
///     }
/// )?;
/// ```
///
/// # Important Notes
///
/// ## Parentheses Requirement
/// When using expressions (like `statement.clone()`) in recursive pod references, you MUST
/// wrap them in parentheses to ensure proper macro parsing:
///
/// ✅ Correct: `predicate(args) => (statement.clone())`  
/// ❌ Incorrect: `predicate(args) => statement.clone()`
///
/// ## Statement Validation
/// The macro automatically validates that statements from recursive pods match the expected
/// predicate type, preventing mismatched statement usage.
///
/// ## Recursive Pod Limit
/// Currently supports up to 2 recursive pods due to macro complexity. For more pods,
/// create intermediate proofs as shown in the publish verification example.
///
/// # Predicate Operations
///
/// Common operations used in inline predicate definitions:
/// - `eq(field, value)` - Assert equality between field and value
/// - `custom(predicate_ref, args...)` - Call custom predicate with arguments
///
/// Fields are referenced as `(pod_variable, "field_name")` tuples.
///
/// # Quick Reference
///
/// ## Before (verbose)
/// ```rust
/// let mut builder = MainPodBuilder::new(&pod_params, vd_set);
/// builder.add_signed_pod(params.identity_pod);
/// let type_check = builder.priv_op(op!(eq, (params.identity_pod, KEY_TYPE), PodType::Signed))?;
/// let username_check = builder.priv_op(op!(eq, (params.identity_pod, "username"), username))?;
/// let pred_ref = batch.predicate_ref_by_name("identity_verified").unwrap();
/// let _verification = builder.pub_op(op!(custom, pred_ref, type_check, username_check))?;
/// let main_pod = builder.prove(prover.as_ref(), &pod_params)?;
/// // ~15+ lines of boilerplate
/// ```
///
/// ## After (with macro)
/// ```rust
/// let main_pod = main_pod!(
///     use_mock_proofs,
///     get_predicate_function,
///     using [params.identity_pod], {
///         identity_verified(params.identity_pod, username) => {
///             eq((params.identity_pod, KEY_TYPE), PodType::Signed),
///             eq((params.identity_pod, "username"), username),
///         }
///     }
/// )?;
/// // ~8 lines, much clearer intent
/// ```
#[macro_export]
macro_rules! main_pod {
    // Unified syntax with optional recursive pods
    ($use_mock:expr, $get_predicate:expr, using [$($pods:expr),*] with recursive [$($recursive_pods:expr),*], {
        $($pred_name:ident($($pred_args:expr),*) => $pred_proof:tt),* $(,)?
    }) => {{
        let pod_params = PodNetProverSetup::get_params();
        let (vd_set, prover) = PodNetProverSetup::create_prover_setup($use_mock)
            .map_err(MainPodError::ProofGeneration)?;

        let mut builder = MainPodBuilder::new(&pod_params, vd_set);

        // Add recursive pods first
        $(
            builder.add_recursive_pod($recursive_pods);
        )*

        // Add all signed pods
        $(
            builder.add_signed_pod($pods);
        )*

        // Parse predicates
        let predicate_input = $get_predicate();
        let batch = parse(&predicate_input, &pod_params, &[])
            .map_err(|e| MainPodError::ProofGeneration(format!("Predicate parsing failed: {}", e)))?
            .custom_batch;

        // Generate proofs for each predicate
        $(
            // Get predicate reference
            let pred_ref = batch.predicate_ref_by_name(stringify!($pred_name))
                .ok_or_else(|| MainPodError::ProofGeneration(format!("Missing {} predicate", stringify!($pred_name))))?;

            // Handle the proof - either from operations or from a previous pod
            let verification_result = main_pod!(@handle_predicate_proof builder, pred_ref, $pred_proof)?;
            let _verification = verification_result;
        )*

        let main_pod = builder.prove(prover.as_ref(), &pod_params)
            .map_err(|e| MainPodError::ProofGeneration(format!("Proof generation failed: {}", e)))?;

        main_pod.pod.verify()
            .map_err(|e| MainPodError::ProofGeneration(format!("Proof verification failed: {}", e)))?;

        Ok::<_, MainPodError>(main_pod)
    }};

    // Unified syntax without recursive pods
    ($use_mock:expr, $get_predicate:expr, using [$($pods:expr),*], {
        $($pred_name:ident($($pred_args:expr),*) => $pred_proof:tt),* $(,)?
    }) => {{
        let pod_params = PodNetProverSetup::get_params();
        let (vd_set, prover) = PodNetProverSetup::create_prover_setup($use_mock)
            .map_err(MainPodError::ProofGeneration)?;

        let mut builder = MainPodBuilder::new(&pod_params, vd_set);

        // Add all signed pods
        $(
            builder.add_signed_pod($pods);
        )*

        // Parse predicates
        let predicate_input = $get_predicate();
        let batch = parse(&predicate_input, &pod_params, &[])
            .map_err(|e| MainPodError::ProofGeneration(format!("Predicate parsing failed: {}", e)))?
            .custom_batch;

        // Generate proofs for each predicate
        $(
            // Get predicate reference
            let pred_ref = batch.predicate_ref_by_name(stringify!($pred_name))
                .ok_or_else(|| MainPodError::ProofGeneration(format!("Missing {} predicate", stringify!($pred_name))))?;

            // Handle the proof - either from operations or from a previous pod
            let verification_result = main_pod!(@handle_predicate_proof builder, pred_ref, $pred_proof)?;
            let _verification = verification_result;
        )*

        let main_pod = builder.prove(prover.as_ref(), &pod_params)
            .map_err(|e| MainPodError::ProofGeneration(format!("Proof generation failed: {}", e)))?;

        main_pod.pod.verify()
            .map_err(|e| MainPodError::ProofGeneration(format!("Proof verification failed: {}", e)))?;

        Ok::<_, MainPodError>(main_pod)
    }};

    // Handler for predicate proofs - from inline operations using curly braces (MUST come before general patterns)
    (@handle_predicate_proof $builder:ident, $pred_ref:expr, { $($op_name:ident($($op_args:expr),*)),* $(,)? }) => {{
        // Generate operations for this predicate
        let mut op_results = Vec::new();
        $(
            let op_result = $builder.priv_op(op!($op_name, $($op_args),*))
                .map_err(|e| MainPodError::ProofGeneration(format!("Operation {} failed: {}", stringify!($op_name), e)))?;
            op_results.push(op_result);
        )*

        // Create the custom predicate proof
        main_pod!(@build_custom_op $builder, $pred_ref, op_results)
    }};

    // Handler for predicate proofs - from a previous pod statement (MUST come after specific patterns)
    (@handle_predicate_proof $builder:ident, $pred_ref:expr, $statement:expr) => {{
        // Validate that the statement matches the expected predicate type
        // Use pattern matching to check if the statement is a Custom variant with the correct predicate
        use pod2::middleware::Statement;

        match $statement {
            Statement::Custom(ref pred, _) => {
                // Check if the predicate reference matches the expected one
                if *pred != $pred_ref {
                    return Err(MainPodError::ProofGeneration(format!(
                        "Statement predicate mismatch: statement predicate does not match expected predicate for this proof"
                    )));
                }
            }
            _ => {
                return Err(MainPodError::ProofGeneration(format!(
                    "Invalid statement type: expected Custom statement with predicate, got {:?}",
                    $statement
                )));
            }
        }

        // Use copy to reference a statement from a recursive pod
        $builder.pub_op(op!(copy, $statement))
            .map_err(|e| MainPodError::ProofGeneration(format!("Copy statement from recursive pod failed: {}", e)))
    }};

    // Helper to build custom operation with variable number of arguments
    (@build_custom_op $builder:ident, $pred_ref:expr, $op_results:expr) => {{
        let ops = $op_results;
        match ops.len() {
            1 => $builder.pub_op(op!(custom, $pred_ref, ops[0].clone())),
            2 => $builder.pub_op(op!(custom, $pred_ref, ops[0].clone(), ops[1].clone())),
            3 => $builder.pub_op(op!(custom, $pred_ref, ops[0].clone(), ops[1].clone(), ops[2].clone())),
            4 => $builder.pub_op(op!(custom, $pred_ref, ops[0].clone(), ops[1].clone(), ops[2].clone(), ops[3].clone())),
            _ => return Err(MainPodError::ProofGeneration("Too many operations for custom predicate".to_string())),
        }
        .map_err(|e| MainPodError::ProofGeneration(format!("Custom predicate operation failed: {}", e)))
    }};
}

/// A macro for verifying MainPods with clean syntax and wildcard support.
///
/// This macro simplifies MainPod verification by automatically handling predicate parsing,
/// statement extraction, and value comparison with support for wildcards.
///
/// # Syntax
/// ```rust
/// verify_main_pod!(
///     main_pod,
///     get_predicate_function(), {
///         predicate_name(expected_arg1, expected_arg2, ...),
///         other_predicate(expected_arg1, _, expected_arg3),  // _ for wildcards
///     }
/// )?;
/// ```
///
/// # Arguments
/// * `main_pod` - The MainPod to verify
/// * `get_predicate_function()` - Function that returns the predicate string
/// * Predicate verification specs - Each predicate with expected argument values
///
/// # Wildcard Support
/// Use `_` to skip verification for specific arguments:
/// ```rust
/// verify_main_pod!(
///     main_pod,
///     get_upvote_verification_predicate(), {
///         upvote_verification(expected_username, _, expected_identity_server_pk)
///     }
/// )?;
/// ```
///
/// # Multiple Predicates
/// You can verify multiple predicates in one call:
/// ```rust
/// verify_main_pod!(
///     main_pod,
///     get_publish_verification_predicate(), {
///         identity_verified(expected_username),
///         document_verified1(_, expected_content_hash, expected_post_id, expected_tags),
///         document_verified2(expected_authors, expected_reply_to, expected_uploader),
///     }
/// )?;
/// ```
///
/// # Returns
/// Returns a `Result<(), MainPodError>` that must be handled with `?` or `.unwrap()`.
///
/// # Example
/// ```rust
/// // Verify upvote verification MainPod
/// verify_main_pod!(
///     &upvote_main_pod,
///     get_upvote_verification_predicate(), {
///         upvote_verification("alice", expected_content_hash, expected_server_pk)
///     }
/// )?;
///
/// // Verify with wildcards (don't care about server key)
/// verify_main_pod!(
///     &upvote_main_pod,
///     get_upvote_verification_predicate(), {
///         upvote_verification("alice", expected_content_hash, _)
///     }
/// )?;
/// ```
///
/// # What It Does
/// The macro automatically:
/// 1. Calls `verify_mainpod_basics(main_pod)?` for signature verification
/// 2. Parses the predicate definition to get predicate references
/// 3. Extracts public statements from the MainPod
/// 4. Compares actual values against expected values (skipping wildcards)
/// 5. Returns appropriate `MainPodError::InvalidValue` on mismatch
///
/// # Quick Reference
///
/// ## Before (verbose)
/// ```rust
/// verify_mainpod_basics(main_pod)?;
/// let (username, content_hash, identity_server_pk) = extract_mainpod_args!(
///     main_pod,
///     get_upvote_verification_predicate(),
///     "upvote_verification",
///     username: as_str,
///     content_hash: as_hash,
///     identity_server_pk: as_public_key
/// )?;
/// if username != expected_username { return Err(...); }
/// if content_hash != expected_content_hash { return Err(...); }
/// // ~10+ lines of boilerplate
/// ```
///
/// ## After (with macro)
/// ```rust
/// verify_main_pod!(
///     main_pod,
///     get_upvote_verification_predicate(), {
///         upvote_verification(expected_username, expected_content_hash, expected_identity_server_pk)
///     }
/// )?;
/// // ~4 lines, much clearer intent
/// ```
#[macro_export]
macro_rules! verify_main_pod {
    ($main_pod:expr, $get_predicate:expr, {
        $($pred_name:ident($($expected:tt),*)),* $(,)?
    }) => {{
        use $crate::mainpod::{MainPodResult, MainPodError, verify_mainpod_basics};
        use pod2::lang::parse;
        use pod2::middleware::Statement;
        use pod_utils::prover_setup::PodNetProverSetup;
        use pod_utils::ValueExt;

        // First verify basic MainPod structure
        verify_mainpod_basics($main_pod)?;

        // Parse predicate to get references
        let params = PodNetProverSetup::get_params();
        let predicate_input = $get_predicate;
        let batch = parse(&predicate_input, &params, &[])
            .map_err(|e| MainPodError::Verification(format!("Predicate parsing failed: {}", e)))?
            .custom_batch;

        $(
            // Get predicate reference for this predicate
            let predicate_ref = batch
                .predicate_ref_by_name(stringify!($pred_name))
                .ok_or_else(|| MainPodError::Verification(format!("Missing {} predicate", stringify!($pred_name))))?;

            // Find the statement for this predicate in the MainPod
            let actual_args = $main_pod
                .public_statements
                .iter()
                .find_map(|stmt| match stmt {
                    Statement::Custom(pred, args) if *pred == predicate_ref => Some(args.as_slice()),
                    _ => None,
                })
                .ok_or_else(|| {
                    MainPodError::Verification(format!("MainPod missing {} statement", stringify!($pred_name)))
                })?;

            // Extract and verify each argument
            let expected_args = [$(verify_main_pod!(@to_value $expected)),*];

            for (i, (actual, expected_opt)) in actual_args.iter().zip(expected_args.iter()).enumerate() {
                if let Some(expected) = expected_opt {
                    if actual != expected {
                        return Err(MainPodError::InvalidValue {
                            field: &format!("{}[{}]", stringify!($pred_name), i),
                            expected: format!("{:?}", expected),
                        });
                    }
                }
            }
        )*

        MainPodResult::Ok(())
    }};

    // Helper to convert values, handling wildcards
    (@to_value _) => { None };
    (@to_value $val:expr) => { Some($val) };
}
