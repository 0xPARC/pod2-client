use std::env;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Whether to use mock proofs instead of real ZK proofs for faster development
    pub mock_proofs: bool,
    /// Port to run the server on
    pub port: u16,
    /// Host to bind the server to
    pub host: String,
    /// Path to the database file
    pub database_path: String,
    /// Path to the content storage directory
    pub content_storage_path: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            mock_proofs: true, // Default to mock proofs for development
            port: 3000,
            host: "0.0.0.0".to_string(), // Bind to all interfaces for deployment
            database_path: "app.db".to_string(),
            content_storage_path: "content".to_string(),
        }
    }
}

impl ServerConfig {
    /// Load configuration from environment variables with fallback to defaults
    pub fn from_env() -> Self {
        let mock_proofs = env::var("PODNET_MOCK_PROOFS")
            .map(|v| v.parse().unwrap_or(true))
            .unwrap_or(true);
        
        let port = env::var("PORT") // Use PORT for Render compatibility
            .or_else(|_| env::var("PODNET_PORT"))
            .map(|v| v.parse().unwrap_or(3000))
            .unwrap_or(3000);
        
        let host = env::var("PODNET_HOST")
            .unwrap_or_else(|_| "0.0.0.0".to_string());
        
        let database_path = env::var("PODNET_DATABASE_PATH")
            .unwrap_or_else(|_| "app.db".to_string());
        
        let content_storage_path = env::var("PODNET_CONTENT_STORAGE_PATH")
            .unwrap_or_else(|_| "content".to_string());
        
        Self {
            mock_proofs,
            port,
            host,
            database_path,
            content_storage_path,
        }
    }

    /// Load configuration (alias for from_env for backward compatibility)
    pub fn load() -> Self {
        let config = Self::from_env();
        tracing::info!("Loaded configuration from environment variables");
        tracing::info!("  Mock proofs: {}", config.mock_proofs);
        tracing::info!("  Host: {}", config.host);
        tracing::info!("  Port: {}", config.port);
        tracing::info!("  Database path: {}", config.database_path);
        tracing::info!("  Content storage path: {}", config.content_storage_path);
        config
    }
}