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
            networking: true,
            authoring: true,
            integration: true,
        }
    }
}

impl FeatureConfig {
    /// Load feature configuration from environment variables or config file
    /// For now, all features are enabled by default
    pub fn load() -> Self {
        // TODO: In the future, this could read from:
        // - Environment variables (POD2_FEATURE_NETWORKING=false)
        // - Config file (config.toml)
        // - Command line arguments
        // - Build-time feature flags
        
        Self::default()
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