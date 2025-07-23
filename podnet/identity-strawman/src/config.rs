use std::env;

#[derive(Debug, Clone)]
pub struct IdentityServerConfig {
    /// Port to run the identity server on
    pub port: u16,
    /// Host to bind the server to
    pub host: String,
    /// Path to the database file for storing user mappings
    pub database_path: String,
    /// Path to the keypair file for server identity
    pub keypair_file: String,
    /// URL of the podnet server to register with
    pub podnet_server_url: String,
}

impl Default for IdentityServerConfig {
    fn default() -> Self {
        Self {
            port: 3001,
            host: "0.0.0.0".to_string(),
            database_path: "identity-users.db".to_string(),
            keypair_file: "identity-server-keypair.json".to_string(),
            podnet_server_url: "http://localhost:3000".to_string(),
        }
    }
}

impl IdentityServerConfig {
    /// Load configuration from environment variables with fallback to defaults
    pub fn from_env() -> Self {
        let port = env::var("IDENTITY_PORT")
            .or_else(|_| env::var("PORT")) // Support standard PORT env var
            .map(|v| v.parse().unwrap_or(3001))
            .unwrap_or(3001);

        let host = env::var("IDENTITY_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());

        let database_path =
            env::var("IDENTITY_DATABASE_PATH").unwrap_or_else(|_| "identity-users.db".to_string());

        let keypair_file = env::var("IDENTITY_KEYPAIR_FILE")
            .unwrap_or_else(|_| "identity-server-keypair.json".to_string());

        let podnet_server_url =
            env::var("PODNET_SERVER_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());

        Self {
            port,
            host,
            database_path,
            keypair_file,
            podnet_server_url,
        }
    }

    /// Load configuration (alias for from_env for consistency with podnet-server)
    pub fn load() -> Self {
        let config = Self::from_env();
        tracing::info!("Loaded identity server configuration from environment variables");
        tracing::info!("  Host: {}", config.host);
        tracing::info!("  Port: {}", config.port);
        tracing::info!("  Database path: {}", config.database_path);
        tracing::info!("  Keypair file: {}", config.keypair_file);
        tracing::info!("  PodNet server URL: {}", config.podnet_server_url);
        config
    }
}
