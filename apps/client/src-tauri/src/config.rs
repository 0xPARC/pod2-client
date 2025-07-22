use std::{
    path::PathBuf,
    sync::{OnceLock, RwLock},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

/// Configuration for enabling/disabling application features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    /// Core POD collection management - viewing, organizing, and basic operations
    pub pod_management: bool,
    /// P2P communication and messaging
    pub p2p: bool,
    /// Creating and signing new PODs
    pub authoring: bool,
    /// External POD Request handling and protocol integration
    pub integration: bool,
    /// FrogCrypto experimental features
    pub frogcrypto: bool,
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            pod_management: true,
            p2p: false,
            authoring: true,
            integration: true,
            frogcrypto: false,
        }
    }
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Path to the database file
    pub path: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: "pod2.db".to_string(),
        }
    }
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Document server URL
    pub document_server: String,
    /// Identity server URL
    pub identity_server: String,
    /// FrogCrypto server URL
    pub frogcrypto_server: String,
    /// Request timeout in seconds
    pub timeout_seconds: u32,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            document_server: "https://pod-server.ghost-spica.ts.net/server".to_string(),
            identity_server: "https://pod-server.ghost-spica.ts.net/identity".to_string(),
            frogcrypto_server: "https://frog-server-q36c.onrender.com".to_string(),
            timeout_seconds: 30,
        }
    }
}

/// UI configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Default theme (auto, light, dark)
    pub default_theme: String,
    /// Default window width
    pub default_window_width: u32,
    /// Default window height
    pub default_window_height: u32,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            default_theme: "auto".to_string(),
            default_window_width: 800,
            default_window_height: 600,
        }
    }
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (debug, info, warn, error)
    pub level: String,
    /// Enable console output
    pub console_output: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            console_output: true,
        }
    }
}

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    /// Feature toggles
    #[serde(default)]
    pub features: FeatureConfig,
    /// Database configuration
    #[serde(default)]
    pub database: DatabaseConfig,
    /// Network configuration
    #[serde(default)]
    pub network: NetworkConfig,
    /// UI configuration
    #[serde(default)]
    pub ui: UiConfig,
    /// Logging configuration
    #[serde(default)]
    pub logging: LoggingConfig,
}

/// Global configuration instance with thread-safe access
static CONFIG: OnceLock<RwLock<AppConfig>> = OnceLock::new();

impl AppConfig {
    /// Get read-only access to the global configuration
    pub fn get() -> std::sync::RwLockReadGuard<'static, AppConfig> {
        CONFIG
            .get()
            .expect("Config not initialized")
            .read()
            .unwrap()
    }

    /// Initialize the global configuration
    pub fn initialize(config: AppConfig) {
        CONFIG
            .set(RwLock::new(config))
            .expect("Config already initialized");
    }

    /// Update the global configuration (for hot reloading)
    pub fn update(config: AppConfig, app_handle: &AppHandle) -> Result<(), String> {
        config.validate()?;

        let config_lock = CONFIG.get().ok_or("Config not initialized")?;
        {
            let mut config_guard = config_lock
                .write()
                .map_err(|e| format!("Failed to acquire write lock: {e}"))?;
            *config_guard = config.clone();
        }

        // Emit config changed event
        app_handle
            .emit("config-changed", &config)
            .map_err(|e| format!("Failed to emit config change event: {e}"))?;

        Ok(())
    }

    /// Load configuration from file
    pub fn load_from_file(config_path: Option<PathBuf>) -> Result<AppConfig, String> {
        match config_path {
            Some(path) => {
                // Load from file - serde will use defaults for missing fields
                let builder =
                    config::Config::builder().add_source(config::File::from(path).required(true));

                let file_config = builder
                    .build()
                    .map_err(|e| format!("Failed to build config from file: {e}"))?;

                file_config
                    .try_deserialize()
                    .map_err(|e| format!("Failed to deserialize config: {e}"))
            }
            None => {
                // No config file specified - use all defaults
                Ok(AppConfig::default())
            }
        }
    }

    /// Apply CLI overrides using dot notation to the configuration
    pub fn apply_overrides(&mut self, overrides: &[String]) -> Result<(), String> {
        for override_str in overrides {
            if let Some((key_path, value)) = override_str.split_once('=') {
                self.apply_single_override(key_path, value)?;
            } else {
                return Err(format!(
                    "Invalid override format: '{}'. Expected format: 'key.path=value'",
                    override_str
                ));
            }
        }
        Ok(())
    }

    /// Apply a single override using dot notation
    fn apply_single_override(&mut self, key_path: &str, value: &str) -> Result<(), String> {
        let parts: Vec<&str> = key_path.split('.').collect();

        match parts.as_slice() {
            ["network", "document_server"] => {
                self.network.document_server = value.to_string();
            }
            ["network", "identity_server"] => {
                self.network.identity_server = value.to_string();
            }
            ["network", "frogcrypto_server"] => {
                self.network.frogcrypto_server = value.to_string();
            }
            ["network", "timeout_seconds"] => {
                self.network.timeout_seconds = value
                    .parse()
                    .map_err(|e| format!("Invalid timeout_seconds value '{}': {}", value, e))?;
            }
            ["features", "pod_management"] => {
                self.features.pod_management = value
                    .parse()
                    .map_err(|e| format!("Invalid pod_management value '{}': {}", value, e))?;
            }
            ["features", "p2p"] => {
                self.features.p2p = value
                    .parse()
                    .map_err(|e| format!("Invalid p2p value '{}': {}", value, e))?;
            }
            ["features", "authoring"] => {
                self.features.authoring = value
                    .parse()
                    .map_err(|e| format!("Invalid authoring value '{}': {}", value, e))?;
            }
            ["features", "integration"] => {
                self.features.integration = value
                    .parse()
                    .map_err(|e| format!("Invalid integration value '{}': {}", value, e))?;
            }
            ["features", "frogcrypto"] => {
                self.features.frogcrypto = value
                    .parse()
                    .map_err(|e| format!("Invalid frogcrypto value '{}': {}", value, e))?;
            }
            ["database", "path"] => {
                self.database.path = value.to_string();
            }
            ["logging", "level"] => {
                if !["debug", "info", "warn", "error"].contains(&value) {
                    return Err(format!(
                        "Invalid logging level '{}'. Must be one of: debug, info, warn, error",
                        value
                    ));
                }
                self.logging.level = value.to_string();
            }
            ["logging", "console_output"] => {
                self.logging.console_output = value
                    .parse()
                    .map_err(|e| format!("Invalid console_output value '{}': {}", value, e))?;
            }
            ["ui", "default_theme"] => {
                if !["auto", "light", "dark"].contains(&value) {
                    return Err(format!(
                        "Invalid default_theme '{}'. Must be one of: auto, light, dark",
                        value
                    ));
                }
                self.ui.default_theme = value.to_string();
            }
            ["ui", "default_window_width"] => {
                self.ui.default_window_width = value.parse().map_err(|e| {
                    format!("Invalid default_window_width value '{}': {}", value, e)
                })?;
            }
            ["ui", "default_window_height"] => {
                self.ui.default_window_height = value.parse().map_err(|e| {
                    format!("Invalid default_window_height value '{}': {}", value, e)
                })?;
            }
            _ => {
                return Err(format!("Unknown config path: '{}'", key_path));
            }
        }

        Ok(())
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        let mut errors = Vec::new();

        // Validate network config
        if self.network.timeout_seconds == 0 {
            errors.push("network.timeout_seconds must be greater than 0".to_string());
        }

        if self.network.document_server.is_empty() {
            errors.push("network.document_server cannot be empty".to_string());
        }

        if self.network.identity_server.is_empty() {
            errors.push("network.identity_server cannot be empty".to_string());
        }

        if self.network.frogcrypto_server.is_empty() {
            errors.push("network.frogcrypto_server cannot be empty".to_string());
        }

        // Validate UI config
        if !["auto", "light", "dark"].contains(&self.ui.default_theme.as_str()) {
            errors.push("ui.default_theme must be 'auto', 'light', or 'dark'".to_string());
        }

        // Validate logging config
        if !["debug", "info", "warn", "error"].contains(&self.logging.level.as_str()) {
            errors.push("logging.level must be 'debug', 'info', 'warn', or 'error'".to_string());
        }

        // Validate database config
        if self.database.path.is_empty() {
            errors.push("database.path cannot be empty".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join(", "))
        }
    }
}

/// Convenience function for accessing configuration
pub fn config() -> std::sync::RwLockReadGuard<'static, AppConfig> {
    AppConfig::get()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults_no_file() {
        // Test that loading with no config file gives us complete defaults
        let config = AppConfig::load_from_file(None).unwrap();

        // Verify all server URLs have defaults
        assert!(!config.network.document_server.is_empty());
        assert!(!config.network.identity_server.is_empty());
        assert!(!config.network.frogcrypto_server.is_empty());
        assert_eq!(
            config.network.document_server,
            "https://pod-server.ghost-spica.ts.net/server"
        );
        assert_eq!(
            config.network.identity_server,
            "https://pod-server.ghost-spica.ts.net/identity"
        );
        assert_eq!(
            config.network.frogcrypto_server,
            "https://frog-server-q36c.onrender.com"
        );

        // Verify other defaults
        assert_eq!(config.database.path, "pod2.db");
        assert!(config.features.pod_management);
        assert!(!config.features.p2p);
    }

    #[test]
    fn test_config_validation() {
        let config = AppConfig::default();
        assert!(config.validate().is_ok());

        // Test validation with invalid values
        let mut invalid_config = AppConfig::default();
        invalid_config.network.document_server = "".to_string();
        assert!(invalid_config.validate().is_err());
    }
}
