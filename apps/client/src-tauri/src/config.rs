use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// Configuration for enabling/disabling application features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    /// Core POD collection management - viewing, organizing, and basic operations
    pub pod_management: bool,
    /// P2P communication and messaging
    pub networking: bool,
    /// Creating and signing new PODs
    pub authoring: bool,
    /// External POD Request handling and protocol integration
    pub integration: bool,
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            pod_management: true,
            networking: false,
            authoring: true,
            integration: true,
        }
    }
}

/// Global cache for feature configuration
static FEATURE_CONFIG: OnceLock<FeatureConfig> = OnceLock::new();

impl FeatureConfig {
    /// Load feature configuration from environment variables (cached)
    /// Falls back to defaults if environment variables are not set
    pub fn load() -> Self {
        FEATURE_CONFIG
            .get_or_init(|| {
                log::info!("Loading feature configuration from environment variables");

                let config = Self {
                    pod_management: Self::get_env_bool("FEATURE_POD_MANAGEMENT", true),
                    networking: Self::get_env_bool("FEATURE_NETWORKING", false),
                    authoring: Self::get_env_bool("FEATURE_AUTHORING", true),
                    integration: Self::get_env_bool("FEATURE_INTEGRATION", true),
                };

                log::info!("Feature configuration loaded: {:?}", config);
                config
            })
            .clone()
    }

    /// Helper to parse boolean from environment variable
    fn get_env_bool(key: &str, default: bool) -> bool {
        match std::env::var(key) {
            Ok(value) => match value.to_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => true,
                "false" | "0" | "no" | "off" => false,
                _ => {
                    log::warn!(
                        "Invalid boolean value '{}' for {}, using default: {}",
                        value,
                        key,
                        default
                    );
                    default
                }
            },
            Err(_) => default,
        }
    }

    /// Check if any features are enabled
    pub fn has_any_enabled(&self) -> bool {
        self.pod_management || self.networking || self.authoring || self.integration
    }

    /// Get a list of enabled feature names
    pub fn enabled_features(&self) -> Vec<&'static str> {
        let mut features = Vec::new();
        if self.pod_management {
            features.push("pod-management");
        }
        if self.networking {
            features.push("networking");
        }
        if self.authoring {
            features.push("authoring");
        }
        if self.integration {
            features.push("integration");
        }
        features
    }
}
