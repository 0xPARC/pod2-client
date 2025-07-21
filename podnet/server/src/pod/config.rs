use axum::http::StatusCode;
use pod2::middleware::{Params, PodProver, VDSet};
use pod_utils::prover_setup::PodNetProverSetup;

/// Configuration for POD-related operations
pub struct PodConfig {
    use_mock: bool,
}

impl PodConfig {
    pub fn new(use_mock: bool) -> Self {
        Self { use_mock }
    }

    /// Get the appropriate prover setup (VDSet and Prover) based on configuration
    pub fn get_prover_setup(&self) -> Result<(&'static VDSet, Box<dyn PodProver>), StatusCode> {
        PodNetProverSetup::create_prover_setup(self.use_mock)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    }

    /// Get default parameters for POD operations
    pub fn get_params(&self) -> Params {
        PodNetProverSetup::get_params()
    }

    /// Get parameters with custom batch size
    pub fn get_params_with_batch_size(&self, batch_size: usize) -> Params {
        PodNetProverSetup::get_params_with_batch_size(batch_size)
    }

    /// Check if using mock proofs
    pub fn is_mock(&self) -> bool {
        self.use_mock
    }
}