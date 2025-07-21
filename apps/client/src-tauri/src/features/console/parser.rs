use std::collections::HashMap;

use super::{aliases::AliasRegistry, types::ConsoleCommand};

/// Parse console command from user input
pub fn parse_console_command(
    input: &str,
    alias_registry: Option<&AliasRegistry>,
) -> Result<ConsoleCommand, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Empty command".to_string());
    }

    // Handle 'exec' prefix for raw Podlang
    if trimmed.starts_with("exec ") {
        let code = trimmed[5..].to_string();
        return Ok(ConsoleCommand::Exec { code });
    }

    // Smart tokenization - handles nested literals with whitespace
    let tokens = smart_tokenize(trimmed);
    if tokens.is_empty() {
        return Err("Empty command".to_string());
    }

    let command_name = tokens[0].clone();

    // Check if this looks like parameter binding (contains '=')
    let has_params = tokens.iter().skip(1).any(|t| t.contains('='));

    if has_params {
        // Definitely an alias with parameters
        let params = parse_parameters(&tokens[1..])?;
        Ok(ConsoleCommand::Alias {
            name: command_name,
            params,
        })
    } else {
        // Check if it's a known alias (even without parameters)
        if let Some(registry) = alias_registry {
            if registry.has_alias(&command_name) {
                // It's a known alias, treat as such (even if no parameters provided)
                Ok(ConsoleCommand::Alias {
                    name: command_name,
                    params: HashMap::new(),
                })
            } else {
                // Not a known alias, treat as built-in
                Ok(ConsoleCommand::BuiltIn {
                    name: command_name,
                    args: tokens[1..].to_vec(),
                })
            }
        } else {
            // No alias registry provided, assume built-in
            Ok(ConsoleCommand::BuiltIn {
                name: command_name,
                args: tokens[1..].to_vec(),
            })
        }
    }
}

/// Smart tokenization that handles Podlang literals with whitespace
fn smart_tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current_token = String::new();
    let mut bracket_depth = 0;
    let mut brace_depth = 0;
    let mut paren_depth = 0;
    let mut in_quotes = false;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '"' if !in_quotes => {
                in_quotes = true;
                current_token.push(ch);
            }
            '"' if in_quotes => {
                in_quotes = false;
                current_token.push(ch);
            }
            '\\' if in_quotes => {
                // Handle escape sequences in strings
                current_token.push(ch);
                if chars.peek().is_some() {
                    current_token.push(chars.next().unwrap());
                }
            }
            '[' if !in_quotes => {
                bracket_depth += 1;
                current_token.push(ch);
            }
            ']' if !in_quotes => {
                bracket_depth -= 1;
                current_token.push(ch);
            }
            '{' if !in_quotes => {
                brace_depth += 1;
                current_token.push(ch);
            }
            '}' if !in_quotes => {
                brace_depth -= 1;
                current_token.push(ch);
            }
            '(' if !in_quotes => {
                paren_depth += 1;
                current_token.push(ch);
            }
            ')' if !in_quotes => {
                paren_depth -= 1;
                current_token.push(ch);
            }
            ' ' | '\t'
                if !in_quotes && bracket_depth == 0 && brace_depth == 0 && paren_depth == 0 =>
            {
                // Safe to split here - we're not inside any nested structure
                if !current_token.is_empty() {
                    tokens.push(current_token.trim().to_string());
                    current_token = String::new();
                }
            }
            _ => {
                current_token.push(ch);
            }
        }
    }

    if !current_token.is_empty() {
        tokens.push(current_token.trim().to_string());
    }

    tokens
}

/// Parse param=value pairs from tokens  
fn parse_parameters(tokens: &[String]) -> Result<HashMap<String, String>, String> {
    let mut params = HashMap::new();

    for token in tokens {
        if let Some(eq_pos) = token.find('=') {
            let key = &token[..eq_pos];
            let value = &token[eq_pos + 1..];

            if key.is_empty() || value.is_empty() {
                return Err(format!("Invalid parameter syntax: {}", token));
            }

            params.insert(key.to_string(), value.to_string());
        } else {
            return Err(format!("Invalid parameter syntax: {}", token));
        }
    }

    Ok(params)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_builtin() {
        let result = parse_console_command("ls zukyc", None).unwrap();
        match result {
            ConsoleCommand::BuiltIn { name, args } => {
                assert_eq!(name, "ls");
                assert_eq!(args, vec!["zukyc"]);
            }
            _ => panic!("Expected BuiltIn command"),
        }
    }

    #[test]
    fn test_simple_alias() {
        let result =
            parse_console_command("zukyc gov=pod_123 age_threshold=1234567890", None).unwrap();
        match result {
            ConsoleCommand::Alias { name, params } => {
                assert_eq!(name, "zukyc");
                assert_eq!(params.get("gov"), Some(&"pod_123".to_string()));
                assert_eq!(params.get("age_threshold"), Some(&"1234567890".to_string()));
            }
            _ => panic!("Expected Alias command"),
        }
    }

    #[test]
    fn test_complex_literal() {
        let result = parse_console_command(
            r#"my_alias param={"key": "value with spaces", "num": 42}"#,
            None,
        )
        .unwrap();
        match result {
            ConsoleCommand::Alias { name, params } => {
                assert_eq!(name, "my_alias");
                assert_eq!(
                    params.get("param"),
                    Some(&r#"{"key": "value with spaces", "num": 42}"#.to_string())
                );
            }
            _ => panic!("Expected Alias command"),
        }
    }

    #[test]
    fn test_exec_command() {
        let result =
            parse_console_command(r#"exec REQUEST(Equal(?pod["field"], "value"))"#, None).unwrap();
        match result {
            ConsoleCommand::Exec { code } => {
                assert_eq!(code, r#"REQUEST(Equal(?pod["field"], "value"))"#);
            }
            _ => panic!("Expected Exec command"),
        }
    }
}
