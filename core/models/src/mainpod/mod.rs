//! MainPod operations for PodNet
//!
//! This module provides consolidated prove and verify functions for all MainPod types
//! used in PodNet, eliminating code duplication and providing consistent interfaces.

pub mod delete;
pub mod publish;
pub mod upvote;
//pub mod upvote_count;

use std::error::Error;

use pod_utils::ValueExt;
use pod2::{
    frontend::{MainPod, SignedPod},
    middleware::{Hash, KEY_SIGNER, Value},
};

/// Common error type for MainPod operations
#[derive(Debug)]
pub enum MainPodError {
    MissingField {
        pod_type: &'static str,
        field: &'static str,
    },
    InvalidValue {
        field: &'static str,
        expected: String,
    },
    ProofGeneration(String),
    Verification(String),
    InvalidSet {
        field: &'static str,
    },
}

impl std::fmt::Display for MainPodError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MainPodError::MissingField { pod_type, field } => {
                write!(f, "{pod_type} pod missing required field: {field}")
            }
            MainPodError::InvalidValue { field, expected } => {
                write!(f, "Invalid value for field {field}: expected {expected}")
            }
            MainPodError::ProofGeneration(msg) => {
                write!(f, "Proof generation failed: {msg}")
            }
            MainPodError::Verification(msg) => {
                write!(f, "Verification failed: {msg}")
            }
            MainPodError::InvalidSet { field } => {
                write!(f, "Invalid set for field: {field}")
            }
        }
    }
}

impl Error for MainPodError {}

/// Result type for MainPod operations
pub type MainPodResult<T> = Result<T, MainPodError>;

/// Extract username from identity pod
pub fn extract_username(identity_pod: &SignedPod) -> MainPodResult<&str> {
    identity_pod
        .get("username")
        .and_then(|v| v.as_str())
        .ok_or(MainPodError::MissingField {
            pod_type: "Identity",
            field: "username",
        })
}

/// Extract user public key from identity pod
pub fn extract_user_public_key(identity_pod: &SignedPod) -> MainPodResult<Value> {
    identity_pod
        .get("user_public_key")
        .cloned()
        .ok_or(MainPodError::MissingField {
            pod_type: "Identity",
            field: "user_public_key",
        })
}

/// Extract identity server public key from identity pod
pub fn extract_identity_server_public_key(identity_pod: &SignedPod) -> MainPodResult<Value> {
    identity_pod
        .get(KEY_SIGNER)
        .cloned()
        .ok_or(MainPodError::MissingField {
            pod_type: "Identity",
            field: KEY_SIGNER,
        })
}

/// Extract content hash from document or upvote pod
pub fn extract_content_hash(pod: &SignedPod, pod_type: &'static str) -> MainPodResult<Hash> {
    pod.get("content_hash")
        .and_then(|v| v.as_hash())
        .ok_or(MainPodError::MissingField {
            pod_type,
            field: "content_hash",
        })
}

/// Extract post ID from document or upvote pod
pub fn extract_post_id(pod: &SignedPod, pod_type: &'static str) -> MainPodResult<Value> {
    pod.get("post_id")
        .cloned()
        .ok_or(MainPodError::MissingField {
            pod_type,
            field: "post_id",
        })
}

pub fn extract_authors(pod: &SignedPod, pod_type: &'static str) -> MainPodResult<Value> {
    pod.get("authors")
        .cloned()
        .ok_or(MainPodError::MissingField {
            pod_type,
            field: "authors",
        })
}

pub fn extract_tags(pod: &SignedPod, pod_type: &'static str) -> MainPodResult<Value> {
    pod.get("tags").cloned().ok_or(MainPodError::MissingField {
        pod_type,
        field: "tags",
    })
}

pub fn extract_reply_to(pod: &SignedPod, pod_type: &'static str) -> MainPodResult<Value> {
    pod.get("reply_to")
        .cloned()
        .ok_or(MainPodError::MissingField {
            pod_type,
            field: "reply_to",
        })
}

/// Verify basic MainPod structure and signature
pub fn verify_mainpod_basics(main_pod: &MainPod) -> MainPodResult<()> {
    main_pod.pod.verify().map_err(|e| {
        MainPodError::Verification(format!("MainPod signature verification failed: {e}"))
    })
}

/// Macro to extract typed arguments from a MainPod's public statements
///
/// Usage:
/// let (username, content_hash, identity_server_pk, post_id, tags) = extract_mainpod_args!(
///     main_pod,
///     get_publish_verification_predicate(),
///     "publish_verification",
///     username: as_str,
///     content_hash: as_hash,
///     identity_server_pk: as_public_key,
///     post_id: as_i64,
///     tags: as_set
/// )?;
#[macro_export]
macro_rules! extract_mainpod_args {
    ($main_pod:expr, $predicate:expr, $statement_name:expr, $($arg_name:ident: $arg_type:ident),* $(,)?) => {
        #[allow(unused_variables, unused_assignments)]
        (|| -> $crate::mainpod::MainPodResult<_> {
            use pod2::lang::parse;
            use pod2::middleware::Statement;
            use pod_utils::prover_setup::PodNetProverSetup;

            // Parse predicate and get reference
            let params = PodNetProverSetup::get_params();
            let predicate_input = $predicate;
            let batch = parse(&predicate_input, &params, &[])
                .map_err(|e| $crate::mainpod::MainPodError::Verification(format!("Predicate parsing failed: {}", e)))?
                .custom_batch;

            let predicate_ref = batch
                .predicate_ref_by_name($statement_name)
                .ok_or_else(|| $crate::mainpod::MainPodError::Verification(format!("Missing {} predicate", $statement_name)))?;

            // Extract statement arguments
            let args = $main_pod
                .public_statements
                .iter()
                .find_map(|stmt| match stmt {
                    Statement::Custom(pred, args) if *pred == predicate_ref => Some(args.as_slice()),
                    _ => None,
                })
                .ok_or_else(|| {
                    $crate::mainpod::MainPodError::Verification(format!("MainPod missing {} statement", $statement_name))
                })?;

            // Extract typed arguments
            let mut index = 0;
            $(
                let $arg_name = args.get(index)
                    .ok_or_else(|| $crate::mainpod::MainPodError::Verification(format!("{} missing argument at index {}", $statement_name, index)))?
                    .$arg_type()
                    .ok_or_else(|| $crate::mainpod::MainPodError::Verification(format!("{} argument '{}' has wrong type", $statement_name, stringify!($arg_name))))?;
                index += 1;
            )*

            Ok(($($arg_name,)*))
        })()
    };
}

#[cfg(test)]
mod tests {

    // Add unit tests for the utility functions here
}
