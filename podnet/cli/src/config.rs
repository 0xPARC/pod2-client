use std::env;

#[derive(Debug, Clone)]
pub struct CliConfig {
    /// Default PodNet server URL
    pub server_url: String,
    /// Default Identity server URL
    pub identity_server_url: String,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            server_url: "http://localhost:3000".to_string(),
            identity_server_url: "http://localhost:3001".to_string(),
        }
    }
}

impl CliConfig {
    /// Load configuration from environment variables with fallback to defaults
    pub fn from_env() -> Self {
        let server_url =
            env::var("PODNET_SERVER_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());

        let identity_server_url = env::var("PODNET_IDENTITY_SERVER_URL")
            .unwrap_or_else(|_| "http://localhost:3001".to_string());

        Self {
            server_url,
            identity_server_url,
        }
    }

    /// Load configuration (alias for from_env)
    pub fn load() -> Self {
        Self::from_env()
    }
}
