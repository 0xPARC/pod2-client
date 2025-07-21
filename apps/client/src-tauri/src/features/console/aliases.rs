use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// TOML configuration structure for aliases
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AliasConfig {
    #[serde(default)]
    pub aliases: HashMap<String, String>,
}

impl Default for AliasConfig {
    fn default() -> Self {
        Self {
            aliases: HashMap::new(),
        }
    }
}

/// Individual alias with metadata
#[derive(Debug, Clone)]
pub struct Alias {
    pub name: String,
    pub template: String,
    pub parameters: Vec<String>, // Extracted wildcard parameters like ?gov, ?age_threshold
}

/// Alias registry for runtime management
#[derive(Debug, Clone)]
pub struct AliasRegistry {
    pub aliases: HashMap<String, Alias>,
    pub config_path: Option<PathBuf>,
}

impl AliasRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            aliases: HashMap::new(),
            config_path: None,
        }
    }

    /// Load aliases from TOML file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read aliases file: {}", path.display()))?;

        let config: AliasConfig = toml::from_str(&content)
            .with_context(|| format!("Failed to parse TOML in: {}", path.display()))?;

        let mut registry = Self::new();
        registry.config_path = Some(path.to_path_buf());

        // Process each alias and extract parameters
        for (name, template) in config.aliases {
            let parameters = extract_parameters(&template);
            let alias = Alias {
                name: name.clone(),
                template,
                parameters,
            };
            registry.aliases.insert(name, alias);
        }

        Ok(registry)
    }

    /// Load aliases from default locations using Tauri paths
    pub fn load_default(app_handle: &AppHandle) -> Result<Self> {
        // Try to find aliases.toml in standard locations
        let possible_paths = get_default_config_paths(app_handle)?;

        log::debug!("Searching for aliases.toml in the following locations:");
        for (i, path) in possible_paths.iter().enumerate() {
            log::debug!("  {}: {}", i + 1, path.display());
            if path.exists() {
                log::info!("Loading aliases from: {}", path.display());
                return Self::load_from_file(path);
            }
        }

        // Return empty registry if no config file found
        log::info!(
            "No aliases.toml found in any of the searched locations, using empty alias registry"
        );
        Ok(Self::new())
    }

    /// Get alias by name
    pub fn get_alias(&self, name: &str) -> Option<&Alias> {
        self.aliases.get(name)
    }

    /// List all alias names
    pub fn list_aliases(&self) -> Vec<&str> {
        self.aliases.keys().map(|s| s.as_str()).collect()
    }

    /// Check if alias exists
    pub fn has_alias(&self, name: &str) -> bool {
        self.aliases.contains_key(name)
    }

    /// Get config file status for display
    pub fn get_config_status(&self) -> String {
        match &self.config_path {
            Some(path) => format!("Loaded from: {}", path.display()),
            None => "No config file loaded".to_string(),
        }
    }

    /// Get all default search paths (for user information)
    pub fn get_search_paths(app_handle: &AppHandle) -> Result<Vec<PathBuf>> {
        get_default_config_paths(app_handle)
    }

    /// Validate alias template syntax (placeholder for now)
    pub fn validate_alias(&self, alias: &Alias) -> Result<()> {
        // TODO: Integrate with existing Podlang parser for validation
        // For now, just check that it's not empty
        if alias.template.trim().is_empty() {
            return Err(anyhow::anyhow!("Alias template cannot be empty"));
        }
        Ok(())
    }
}

/// Extract parameter names from alias template
/// Looks for patterns like ?param_name in the template
fn extract_parameters(template: &str) -> Vec<String> {
    let mut parameters = Vec::new();
    let mut chars = template.chars().peekable();

    while let Some(&ch) = chars.peek() {
        if ch == '?' {
            chars.next(); // consume '?'
            let mut param = String::new();

            // Collect parameter name (alphanumeric + underscore)
            while let Some(&ch) = chars.peek() {
                if ch.is_alphanumeric() || ch == '_' {
                    param.push(ch);
                    chars.next();
                } else {
                    break;
                }
            }

            if !param.is_empty() && !parameters.contains(&param) {
                parameters.push(param);
            }
        } else {
            chars.next();
        }
    }

    parameters
}

/// Get default configuration file paths to search using Tauri APIs
fn get_default_config_paths(app_handle: &AppHandle) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();

    // Primary: Tauri app config directory
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .with_context(|| "Failed to get app config directory")?;
    paths.push(config_dir.join("aliases.toml"));

    // Fallback: Current working directory
    paths.push(PathBuf::from("aliases.toml"));

    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_parameters() {
        let template = r#"
        REQUEST(
            NotContains(?sanctions["sanctionList"], ?gov["idNumber"])
            Lt(?gov["dateOfBirth"], ?age_threshold)
            Equal(?pay["startDate"], ?start_date)
        )
        "#;

        let params = extract_parameters(template);
        assert_eq!(params.len(), 4);
        assert!(params.contains(&"sanctions".to_string()));
        assert!(params.contains(&"gov".to_string()));
        assert!(params.contains(&"age_threshold".to_string()));
        assert!(params.contains(&"start_date".to_string()));
    }

    #[test]
    fn test_parse_toml_config() {
        let toml_content = r#"
        [aliases]
        zukyc = """
        REQUEST(
            NotContains(?sanctions["sanctionList"], ?gov["idNumber"])
            Lt(?gov["dateOfBirth"], ?age_threshold)
        )
        """
        test = "REQUEST(Equal(?a, ?b))"
        "#;

        let config: AliasConfig = toml::from_str(toml_content).unwrap();
        assert_eq!(config.aliases.len(), 2);
        assert!(config.aliases.contains_key("zukyc"));
        assert!(config.aliases.contains_key("test"));
    }
}
