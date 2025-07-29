use thiserror::Error;

#[derive(Error, Debug)]
pub enum SolverError {
    #[error("Internal solver error: {0}")]
    Internal(String),
    #[error("Failed to parse datalog: {0}")]
    Parsing(String),
}

impl SolverError {
    pub(crate) fn arg_mismatch(
        pred: pod2::middleware::NativePredicate,
        expected: usize,
        actual: usize,
    ) -> SolverError {
        SolverError::Internal(format!(
            "Wrong number of arguments for {pred:?}: expected {expected}, got {actual}"
        ))
    }
}
