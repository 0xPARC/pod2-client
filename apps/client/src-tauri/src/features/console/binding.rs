use std::collections::HashMap;

use anyhow::Result;

use super::aliases::Alias;

/// Parameter binding engine for wildcard substitution
#[derive(Debug, Clone)]
pub struct ParameterBinder {
    // Reserved for future extension
}

impl ParameterBinder {
    /// Create a new parameter binder
    pub fn new() -> Self {
        Self {}
    }

    /// Bind parameters to an alias template, returning the resolved Podlang code
    /// Parameters are optional - unbound parameters remain as free variables for the solver
    pub fn bind_parameters(
        &self,
        alias: &Alias,
        provided_params: &HashMap<String, String>,
    ) -> Result<String> {
        // Check for extra parameters (parameters not defined in the alias)
        let extra_params: Vec<_> = provided_params
            .keys()
            .filter(|param| !alias.parameters.contains(param))
            .collect();

        if !extra_params.is_empty() {
            return Err(anyhow::anyhow!(
                "Unknown parameters for alias '{}': {}",
                alias.name,
                extra_params
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        // Add equality constraints inside the REQUEST block for provided parameters
        // Leave unbound parameters as free variables for the solver
        let mut result = alias.template.clone();

        if !provided_params.is_empty() {
            // Find the REQUEST( opening and insert equality constraints
            if let Some(request_start) = result.find("REQUEST(") {
                let insert_pos = request_start + "REQUEST(".len();

                // Build equality constraints for provided parameters
                let mut constraints = Vec::new();
                for (param_name, param_value) in provided_params {
                    constraints.push(format!("    Equal(?{}, {})", param_name, param_value));
                }

                // Insert constraints after REQUEST( with proper formatting
                let constraint_block = if !constraints.is_empty() {
                    format!("\n{}\n", constraints.join("\n"))
                } else {
                    String::new()
                };

                result.insert_str(insert_pos, &constraint_block);
            } else {
                return Err(anyhow::anyhow!(
                    "Alias template for '{}' does not contain a REQUEST block",
                    alias.name
                ));
            }
        }

        Ok(result)
    }

    /// Validate parameter values (placeholder for future type checking)
    pub fn validate_parameter_value(&self, param_name: &str, param_value: &str) -> Result<()> {
        // TODO: Add parameter type validation based on Podlang types
        // For now, just check basic constraints

        if param_value.trim().is_empty() {
            return Err(anyhow::anyhow!(
                "Parameter '{}' cannot be empty",
                param_name
            ));
        }

        // Basic POD ID validation (if it looks like a POD ID)
        if param_value.starts_with("pod_") {
            if param_value.len() < 8 {
                return Err(anyhow::anyhow!(
                    "Parameter '{}' looks like a POD ID but is too short: '{}'",
                    param_name,
                    param_value
                ));
            }
        }

        // Basic numeric validation (if it looks like a number)
        if param_value.chars().all(|c| c.is_ascii_digit()) {
            // Validate as potential timestamp or numeric value
            if let Err(_) = param_value.parse::<u64>() {
                return Err(anyhow::anyhow!(
                    "Parameter '{}' looks numeric but cannot be parsed: '{}'",
                    param_name,
                    param_value
                ));
            }
        }

        Ok(())
    }

    /// Get parameter suggestions for autocomplete (future feature)
    pub fn get_parameter_suggestions(&self, _alias: &Alias, _partial_param: &str) -> Vec<String> {
        // TODO: Implement smart parameter suggestions
        // - Recent parameter values from history
        // - POD IDs from current collection
        // - Common timestamp formats
        // - Validation-aware suggestions
        Vec::new()
    }

    /// Analyze parameter usage patterns (for optimization)
    pub fn analyze_parameters(&self, alias: &Alias) -> ParameterAnalysis {
        let mut analysis = ParameterAnalysis::new();

        for param in &alias.parameters {
            // Count parameter usage frequency in template
            let wildcard = format!("?{}", param);
            let usage_count = alias.template.matches(&wildcard).count();
            analysis.parameter_usage.insert(param.clone(), usage_count);

            // Detect parameter context patterns
            if alias.template.contains(&format!("?{}[", param)) {
                analysis.indexed_parameters.push(param.clone());
            }

            // Detect potential type hints from context
            if alias.template.contains(&format!("Lt(?{}", param))
                || alias.template.contains(&format!("Gt(?{}", param))
            {
                analysis.numeric_parameters.push(param.clone());
            }
        }

        analysis
    }
}

/// Analysis results for parameter patterns in an alias
#[derive(Debug, Clone)]
pub struct ParameterAnalysis {
    pub parameter_usage: HashMap<String, usize>, // Parameter -> usage count
    pub indexed_parameters: Vec<String>,         // Parameters used with array indexing
    pub numeric_parameters: Vec<String>,         // Parameters likely to be numeric
}

impl ParameterAnalysis {
    fn new() -> Self {
        Self {
            parameter_usage: HashMap::new(),
            indexed_parameters: Vec::new(),
            numeric_parameters: Vec::new(),
        }
    }
}

/// Helper function to parse parameter string "key=value key2=value2" into HashMap
pub fn parse_parameter_string(param_string: &str) -> Result<HashMap<String, String>> {
    let mut params = HashMap::new();

    if param_string.trim().is_empty() {
        return Ok(params);
    }

    // Split by whitespace, but respect quoted values
    let mut chars = param_string.chars().peekable();
    let mut current_token = String::new();
    let mut in_quotes = false;
    let mut tokens = Vec::new();

    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                current_token.push(ch);
            }
            ' ' | '\t' if !in_quotes => {
                if !current_token.is_empty() {
                    tokens.push(current_token.clone());
                    current_token.clear();
                }
            }
            _ => {
                current_token.push(ch);
            }
        }
    }

    if !current_token.is_empty() {
        tokens.push(current_token);
    }

    // Parse each token as key=value
    for token in tokens {
        let parts: Vec<&str> = token.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!(
                "Invalid parameter format '{}'. Expected 'key=value'",
                token
            ));
        }

        let key = parts[0].trim().to_string();
        let mut value = parts[1].trim().to_string();

        // Remove quotes if present
        if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
            value = value[1..value.len() - 1].to_string();
        }

        if key.is_empty() {
            return Err(anyhow::anyhow!("Empty parameter name in '{}'", token));
        }

        params.insert(key, value);
    }

    Ok(params)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::console::aliases::Alias;

    #[test]
    fn test_parameter_binding() {
        let alias = Alias {
            name: "test".to_string(),
            template: "REQUEST(Equal(?gov, ?value))".to_string(),
            parameters: vec!["gov".to_string(), "value".to_string()],
        };

        let mut params = HashMap::new();
        params.insert("gov".to_string(), "pod_123".to_string());
        params.insert("value".to_string(), "test_value".to_string());

        let binder = ParameterBinder::new();
        let result = binder.bind_parameters(&alias, &params).unwrap();

        assert_eq!(result, "REQUEST(\n    Equal(?gov, pod_123)\n    Equal(?value, test_value)\nEqual(?gov, ?value))");
    }

    #[test]
    fn test_missing_parameter_left_unbound() {
        let alias = Alias {
            name: "test".to_string(),
            template: "REQUEST(Equal(?gov, ?value))".to_string(),
            parameters: vec!["gov".to_string(), "value".to_string()],
        };

        let mut params = HashMap::new();
        params.insert("gov".to_string(), "pod_123".to_string());
        // Missing "value" parameter - should be left as ?value for solver

        let binder = ParameterBinder::new();
        let result = binder.bind_parameters(&alias, &params).unwrap();

        assert_eq!(
            result,
            "REQUEST(\n    Equal(?gov, pod_123)\nEqual(?gov, ?value))"
        );
    }

    #[test]
    fn test_no_parameters_all_unbound() {
        let alias = Alias {
            name: "zukyc".to_string(),
            template: "REQUEST(NotContains(?sanctions[\"sanctionList\"], ?gov[\"idNumber\"]))"
                .to_string(),
            parameters: vec!["sanctions".to_string(), "gov".to_string()],
        };

        let params = HashMap::new(); // No parameters provided

        let binder = ParameterBinder::new();
        let result = binder.bind_parameters(&alias, &params).unwrap();

        // Should leave all parameters as free variables - no constraints added
        assert_eq!(
            result,
            "REQUEST(NotContains(?sanctions[\"sanctionList\"], ?gov[\"idNumber\"]))"
        );
    }

    #[test]
    fn test_parse_parameter_string() {
        let result = parse_parameter_string("gov=pod_123 age=567890123").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result.get("gov"), Some(&"pod_123".to_string()));
        assert_eq!(result.get("age"), Some(&"567890123".to_string()));
    }

    #[test]
    fn test_parse_quoted_parameters() {
        let result = parse_parameter_string(r#"name="John Doe" file="path with spaces""#).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result.get("name"), Some(&"John Doe".to_string()));
        assert_eq!(result.get("file"), Some(&"path with spaces".to_string()));
    }
}
