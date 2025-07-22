use pod2::{backends::plonky2::mainpod::get_common_data, middleware::Params};

/// Warm the MainPod CommonCircuitData cache by triggering the expensive computation
/// during app startup instead of on first use.
pub fn warm_mainpod_cache() {
    log::info!("Starting MainPod cache warming...");

    // Call get_common_data with default Params to trigger OnceLock initialization
    // This pre-generates the expensive CommonCircuitData computation
    let params = Params::default();

    match get_common_data(&params) {
        Ok(_) => {
            log::info!("Successfully warmed MainPod CommonCircuitData cache");
        }
        Err(e) => {
            log::warn!("Failed to warm MainPod cache: {e}");
        }
    }
}
