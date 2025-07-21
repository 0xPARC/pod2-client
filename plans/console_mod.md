# Console Mode Implementation Specification

## Overview

This document outlines the implementation of a "console mode" for the POD2 client app - an IRC/terminal-style interface that provides a command-line experience for POD operations. Users will be able to define POD request templates in a TOML configuration file and execute them via slash commands with named parameters.

### Inspiration

The design is inspired by classic IRC clients (particularly mIRC) and shell terminals, providing:
- A scrollable wall-of-text output area
- A command prompt at the bottom 
- Slash-command system (`/command args...`)
- Configuration file for defining custom command aliases
- Command history and autocomplete

## Current System Integration

### Existing POD Request Infrastructure

The console mode will build upon the existing POD request system:

**Language**: Podlang with `REQUEST()` blocks containing predicates
```podlang
REQUEST(
    NotContains(?sanctions["sanctionList"], ?gov["idNumber"])
    Lt(?gov["dateOfBirth"], 1234567890)
    Equal(?pay["startDate"], 1706367566)  
    Equal(?gov["socialSecurityNumber"], ?pay["socialSecurityNumber"])
)
```

**Variable System**: 
- Named variables use `?variable_name` syntax (e.g., `?gov`, `?sanctions`, `?Distance`)
- Field access via `?variable["field"]` syntax
- Private intermediate values via `private:` keyword

**Existing RPC Commands**:
- `execute_code_command(code, mock)` - Execute Podlang against all PODs
- `validate_code_command(code)` - Validate syntax without execution
- `submit_pod_request(request)` - Submit POD request for external integration

**Current UI**: Monaco editor with Podlang syntax highlighting, validation diagnostics, and visual results

## TOML Alias System Design

### Configuration File Format

Store aliases in `podnet.toml` in the Tauri app data directory:

```toml
[aliases]
# ZuKYC example from solver tests  
zukyc = """
REQUEST(
    NotContains(?sanctions["sanctionList"], ?gov["idNumber"])
    Lt(?gov["dateOfBirth"], ?age_threshold)
    Equal(?pay["startDate"], ?start_date)
    Equal(?gov["socialSecurityNumber"], ?pay["socialSecurityNumber"])
)
"""

# EthDOS example from solver tests
ethdos = """  
use _, _, _, eth_dos from ?batch

REQUEST(
    eth_dos(?src, ?dst, ?Distance)
)
"""

# Custom predicate example
age_check = """
REQUEST(
    Lt(?person["dateOfBirth"], ?threshold)
)
"""
```

### Wildcard Parameter Binding System

**Wildcard Variables**: Use existing `?variable` syntax in alias definitions
**Parameter Binding**: Command-line syntax like `/zukyc gov=pod_123 age_threshold=1234567890`
**Automatic Binding**: Console binds command parameters to matching `?variable` names in the alias

### Built-in Aliases

Include sample aliases based on existing test cases:
- `zukyc` - Age and sanctions verification
- `ethdos` - Ethereum degrees of separation  
- Basic predicates for common operations

## Command System Design

### Slash Command Syntax

```
/command [param=value] [param=value] [--options]
```

Examples:
```
/zukyc gov=pod_abc123 sanctions=pod_def456 age_threshold=567890123
/ethdos src=PublicKey(Alice) dst=PublicKey(Bob) batch=0xabc123def456
/age_check person=pod_xyz789 threshold=946684800
```

### Built-in Commands

**Core Commands**:
- `help [command]` - Show help for specific command or list all commands
- `clear` - Clear console output
- `history [--search term]` - Show/search command history
- `reload` - Reload TOML configuration file

**POD Management (Unix-style)**:  
- `ls [folder]` - List PODs (all PODs or PODs in specific folder)
- `mv <pod_id> <folder>` / `move <pod_id> <folder>` - Move POD to different folder
- `rm <pod_id>` / `delete <pod_id>` - Delete POD 
- `cp <pod_id> <new_name>` / `copy <pod_id> <new_name>` - Duplicate POD in current folder
- `mkdir <folder>` - Create new folder
- `rmdir <folder>` - Delete folder (if empty)
- `pwd` - Show current working folder
- `cd <folder>` - Change to different folder (affects where new PODs are saved)

**POD Information**:
- `cat <pod_id>` / `show <pod_id>` - Display POD contents
- `file <pod_id>` - Show POD type and metadata
- `find <pattern>` - Search PODs by name or content
- `grep <pattern> [folder]` - Search within POD contents
- `validate <alias> [args...]` - Validate alias without executing

**Configuration**:
- `edit-config [core|aliases]` - Open configuration file in system editor (default: aliases)
- `save <name> <query>` - Save ad-hoc Podlang query as new alias
- `alias [name]` - Show alias definition or list all aliases
- `set <setting> <value>` - Configure core settings (saved to podnet.toml)
- `log <on|off> <event_type>` - Control GUI event logging
- `reload` - Reload both configuration files

**Execution**:
- `<alias> [args...]` - Execute alias with wildcard parameter binding (PODs saved to current folder)

### Command Parsing Architecture

Console commands use **direct Podlang literal parsing** by reusing the existing Pest parser infrastructure. No new parsing logic is needed - we leverage the robust `literal_value` grammar rules already implemented.

#### Simple Command Grammar

**Command Syntax**:
```
command := builtin_command | alias_command | exec_command
builtin_command := builtin_name args*
alias_command := alias_name parameter_binding*
exec_command := "exec" podlang_code
parameter_binding := identifier "=" podlang_literal
podlang_literal := <existing literal_value rule from grammar>
```

**Command Types**:
- **Built-in Commands**: `ls zukyc`, `cd work`, `pwd` (simple string arguments)
- **Alias Commands**: `zukyc gov=pod_123 age_threshold=1234567890` (Podlang literal parameters)  
- **Raw Podlang**: `exec REQUEST(Equal(?pod["field"], "value"))` (direct code execution)

#### Direct Podlang Literal Support

**Reuse Existing Parser**: Parameter values use the existing `Rule::test_literal_value` from the Podlang grammar:

```rust
use pod2::lang::parser::{PodlangParser, Rule};
use pest::Parser;

fn validate_parameter_value(value: &str) -> Result<(), String> {
    match PodlangParser::parse(Rule::test_literal_value, value) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Invalid literal: {}", e))
    }
}
```

**Supported Literal Types** (inherited from existing grammar):
- **Simple**: `123`, `true`, `"string value"`, `pod_abc123`
- **Complex**: `["a", 123, true]`, `{"key": "value"}`, `#[1, 2, 3]`  
- **Nested**: `{"users": [{"name": "Alice"}, {"name": "Bob"}], "active": true}`
- **Advanced**: `PublicKey(base58string)`, `0x1234...`, `Raw(0x...)`

#### TOML Multi-line Aliases (No Escaping)

**Clean Alias Storage**: Use TOML's native multi-line strings to avoid any escaping:

```toml
[aliases]
zukyc = """
REQUEST(
    NotContains(?sanctions["sanctionList"], ?gov["idNumber"])
    Lt(?gov["dateOfBirth"], ?age_threshold)
    Equal(?gov["socialSecurityNumber"], ?pay["socialSecurityNumber"])
)
"""

complex_query = """
REQUEST(
    Equal(?data["permissions"], ["read", "write", "admin"])
    Contains(?data["roles"], #["user", "verified"])  
    Gt(?data["created_at"], 1640995200)
)
"""
```

#### Real-time Parameter Validation

**Live Feedback**: Parse parameter values as user types using existing parser:

```console
> my_alias param={"key": 
  ▸ Parsing parameter value...
> my_alias param={"key": "value"}
  ✓ Valid dictionary literal
> my_alias param={"key": invalid!
  ✗ Invalid parameter 'param': Expected string, got invalid!
         ^^^^^
```

**Error Examples**:
```console
> zukyc gov=invalid_format!
✗ Error in parameter 'gov': Invalid character in literal
  Expected: POD ID (0x...) or string ("...")
  
> my_alias data={"key": unclosed
✗ Error in parameter 'data': Incomplete dictionary
  Missing closing brace }

> zukyc gov=pod_123
✗ Missing required parameters for alias 'zukyc'
  Provided: ?gov ✓  
  Required: ?age_threshold, ?sanctions
```

### Parameter Binding Logic

1. **Tokenize command**: Simple string splitting to extract command name and `param=value` pairs
2. **Validate parameters**: Use existing `PodlangParser::parse(Rule::test_literal_value, value)` for each parameter
3. **Load alias template**: Retrieve raw Podlang from TOML multi-line string (no escaping needed)
4. **Bind wildcards**: Direct string replacement of `?key` wildcards with validated literal values
5. **Execute**: Use existing `execute_code_command()` with parameter-bound Podlang code  
6. **Display results**: Show execution results, errors, and any generated PODs

**Simplified Flow**: `command` → **tokenize** → **validate literals** → **string substitution** → **existing execution**

## UI/UX Design

### Console Interface Layout

```
┌─ Console ──────────────────────────────────────────┐
│ [14:32:15] zukyc/ > zukyc gov=pod_123 age_threshold=567890123
│ [14:32:15] Executing alias: zukyc                  │  
│ [14:32:15] Binding wildcards: ?gov←pod_123 ?age_threshold←567890123
│            REQUEST(                                │
│                NotContains(?sanctions["sanctionList"], ?gov["idNumber"])
│                Lt(?gov["dateOfBirth"], ?age_threshold)
│                Equal(?gov["socialSecurityNumber"], ?pay["socialSecurityNumber"])
│            )                                       │
│ [14:32:16] ✓ Execution successful                  │
│ [14:32:16] Main POD: pod_new456 saved to current folder (zukyc/)
│ [14:32:16] Used PODs: pod_123, pod_def456, pod_ghi789
│ [14:32:16] Proof: 847 operations, 3 inputs, 1 output
│ [14:32:20] 📥 POD imported via GUI: pod_xyz999 → default/
│ [14:32:25] zukyc/ > help zukyc                     │
│ [14:32:25] zukyc - Age and sanctions verification  │
│            Wildcards: ?gov ?sanctions ?age_threshold ?pay ?start_date
│            Example: zukyc gov=pod_123 age_threshold=946684800
│            Note: Generated PODs saved to current folder
│ [14:32:35] zukyc/ > █                              │
└────────────────────────────────────────────────────┘
```

### Component Structure

**ConsoleView** - Main container with terminal-like interface
- Dark/light theme support following existing app patterns
- Resizable if needed (but primarily full-screen experience)
- Status bar showing current configuration file status

**ConsoleOutput** - Pure text scrollable area
- Plain text output with full drag-selection support
- Basic ANSI-style color coding for different message types
- Clickable hyperlinks for POD IDs (opens PodViewer in new view)
- Clickable links for "View full results" that open EditorResults component

**ConsoleInput** - Simple command input
- Command history (up/down arrows)
- Tab autocomplete for commands, aliases, and POD IDs
- Basic input validation (no syntax highlighting)

**ConsoleHistory** - Persistent command history
- Store in Tauri's app data directory
- Search and replay previous commands
- Import/export history for sharing

### Integration with Existing UI

**Sidebar Navigation**: Add "Console" option to existing sidebar
**Feature Flags**: Respect existing authoring/integration feature flags
**Theming**: Use existing CSS variables and theme system
**State Management**: Integrate with existing app state for POD access

## Implementation Phases

### Phase 1: Basic Console Infrastructure
1. Create console UI components (ConsoleView, ConsoleInput, ConsoleOutput)
2. Implement basic slash command parsing 
3. Add console option to sidebar navigation
4. Integrate with existing theme system

### Phase 2: TOML Configuration System  
1. Add TOML parsing library to project dependencies
2. Implement configuration file reading/writing via Tauri filesystem APIs
3. Create default configuration with sample aliases
4. Add configuration reload and validation

### Phase 3: Command Parsing & Parameter Binding System
1. **Two-Stage Parser Implementation**:
   - Create console command parser (`console/parser.rs`) for shell-style syntax
   - Implement tokenization with proper quote and escape handling
   - Add command type detection (built-in vs alias vs exec)

2. **Parameter Binding Engine** (`console/binding.rs`):
   - Implement parameter value parsing using existing Podlang literal system
   - Create wildcard template substitution (`?key` ← `key=value`)
   - Add support for all Podlang literal types (strings, arrays, dicts, etc.)

3. **Integration with Existing Infrastructure**:
   - Leverage existing `lang::parse()` for parameter value validation
   - Reuse `execute_code_command()` and `validate_code_command()` for execution
   - Maintain compatibility with current error reporting system

4. **Comprehensive Error Handling**:
   - Shell-level parsing errors (unclosed quotes, invalid syntax)
   - Parameter-level parsing errors (invalid literal formats)  
   - Template binding errors (missing parameters, type mismatches)

### Phase 4: Enhanced Commands & Event System
1. Implement built-in commands (help, clear, ls, mv, etc.)
2. Add command history with persistence
3. Implement autocomplete for commands and POD IDs
4. Add result formatting for different command types
5. Create event bus system for GUI-to-console logging
6. Add timestamp display and configuration
7. Implement event filtering and logging controls

### Phase 5: Advanced Features
1. Configuration editor integration  
2. Template import/export functionality
3. Command chaining and scripting capabilities
4. Performance optimizations and polish

## Technical Architecture

### File Structure

**Rust Backend (Primary Logic)**:
```
src-tauri/src/features/console/
├── mod.rs                   # Module declarations and public API
├── commands.rs              # Command parsing and built-in commands
├── aliases.rs               # TOML alias management and loading
├── binding.rs               # Wildcard parameter binding logic  
├── history.rs               # Command history storage and retrieval
├── events.rs                # Event bus and message logging
├── session.rs               # Console session state management
└── types.rs                 # Console data types and structures
```

**JavaScript Frontend (Display Layer)**:
```
src/components/console/
├── ConsoleView.tsx          # Main container and layout
├── ConsoleOutput.tsx        # Message display and scrolling
├── ConsoleInput.tsx         # Input field and basic interaction
└── types.ts                 # Frontend types (mirrors Rust types)

src/lib/console/
├── rpc.ts                   # Tauri RPC client bindings
└── utils.ts                 # Frontend utilities (formatting, etc.)
```

### Simplified Command Parsing Implementation

**Simple Command Tokenizer** (`console/parser.rs`):
```rust
use std::collections::HashMap;
use pod2::lang::parser::{PodlangParser, Rule};
use pest::Parser;

#[derive(Debug, Clone, PartialEq)]
pub enum ConsoleCommand {
    BuiltIn { name: String, args: Vec<String> },
    Alias { name: String, params: HashMap<String, String> },
    Exec { code: String },
}

/// Simple command parsing - just tokenize and identify patterns
pub fn parse_console_command(input: &str) -> Result<ConsoleCommand, String> {
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
    let command_name = tokens[0].clone();
    
    // Check if this looks like parameter binding (contains '=')
    if tokens.iter().any(|t| t.contains('=')) {
        let params = parse_parameters(&tokens[1..])?;
        Ok(ConsoleCommand::Alias { name: command_name, params })
    } else {
        Ok(ConsoleCommand::BuiltIn { name: command_name, args: tokens[1..].to_vec() })
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
                if let Some(&next_ch) = chars.peek() {
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
            ' ' | '\t' if !in_quotes && bracket_depth == 0 && brace_depth == 0 && paren_depth == 0 => {
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

/// Parse param=value pairs, validating values as Podlang literals
fn parse_parameters(tokens: &[String]) -> Result<HashMap<String, String>, String> {
    let mut params = HashMap::new();
    
    for token in tokens {
        if let Some(eq_pos) = token.find('=') {
            let key = &token[..eq_pos];
            let value = &token[eq_pos + 1..];
            
            // Validate parameter value using existing Podlang parser
            validate_parameter_value(value)
                .map_err(|e| format!("Invalid value for parameter '{}': {}", key, e))?;
            
            params.insert(key.to_string(), value.to_string());
        } else {
            return Err(format!("Invalid parameter syntax: {}", token));
        }
    }
    
    Ok(params)
}
```

**Parameter Validation using Existing Parser**:
```rust
/// Validate parameter value as Podlang literal using existing grammar
fn validate_parameter_value(value: &str) -> Result<(), String> {
    match PodlangParser::parse(Rule::test_literal_value, value) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Invalid Podlang literal: {}", e))
    }
}

/// Real-time validation for UI feedback
pub fn validate_parameter_realtime(value: &str) -> ValidationResult {
    match PodlangParser::parse(Rule::test_literal_value, value) {
        Ok(_) => ValidationResult::Valid,
        Err(e) => ValidationResult::Invalid { 
            error: format!("{}", e),
            suggestion: suggest_fix_for_literal_error(&e)
        }
    }
}
```

**Template Binding** (`console/binding.rs`):
```rust
use std::collections::HashMap;
use regex::Regex;

/// Simple wildcard substitution in TOML alias templates
pub fn bind_parameters_to_template(
    template: &str,
    params: &HashMap<String, String>,
) -> Result<String, String> {
    let mut bound_template = template.to_string();
    
    // Find all ?wildcard patterns
    let wildcard_regex = Regex::new(r"\?([a-zA-Z_][a-zA-Z0-9_]*)")?;
    let wildcards: Vec<_> = wildcard_regex
        .captures_iter(template)
        .map(|cap| cap[0].to_string()) // Full ?wildcard
        .collect();
    
    // Replace each wildcard with its parameter value
    for wildcard in wildcards {
        let param_name = &wildcard[1..]; // Remove '?'
        
        if let Some(param_value) = params.get(param_name) {
            // Direct string replacement - value already validated as Podlang literal
            bound_template = bound_template.replace(&wildcard, param_value);
        } else {
            return Err(format!(
                "Missing required parameter: {} (available: {})",
                wildcard,
                params.keys().map(|k| format!("?{}", k)).collect::<Vec<_>>().join(", ")
            ));
        }
    }
    
    Ok(bound_template)
}
```

**Integration with Existing System**:
```rust
// In console/commands.rs  
pub async fn execute_alias_command(
    state: &AppState,
    alias_name: &str,
    params: HashMap<String, String>,
) -> Result<ConsoleResponse, String> {
    // Load raw Podlang from TOML (no escaping needed)
    let alias_template = load_alias_from_toml(alias_name)?;
    
    // Simple string substitution of validated parameters
    let bound_podlang = bind_parameters_to_template(&alias_template, &params)?;
    
    // Use existing execution infrastructure directly
    match crate::features::authoring::execute_code_command(
        state.clone(), 
        bound_podlang.clone(),
        false // not mock
    ).await {
        Ok(result) => Ok(ConsoleResponse::Success { result, bound_code: bound_podlang }),
        Err(error) => Ok(ConsoleResponse::Error { error, bound_code: Some(bound_podlang) }),
    }
}
```

**Key Benefits**:
- **Reuses existing parser**: Zero new parsing logic, leverages proven `Rule::test_literal_value`
- **No escaping complexity**: TOML multi-line strings + direct Podlang literals  
- **Real-time validation**: Same parser provides immediate feedback as user types
- **Smart tokenization**: Handles whitespace in literals correctly with bracket/quote tracking
- **Full literal support**: Inherits all Podlang types automatically (arrays, dicts, PublicKey, etc.)

### Command Parsing Examples

**Simple Alias Command**:
```console
> zukyc gov=pod_abc123 age_threshold=1234567890
```
Parse result:
```rust
ConsoleCommand::Alias {
    name: "zukyc".to_string(),
    params: {
        "gov": "pod_abc123".to_string(),  // ✓ Validated as identifier/POD ID
        "age_threshold": "1234567890".to_string(), // ✓ Validated as integer
    }
}
```

**Complex Parameter Values (Direct Podlang)**:
```console
> my_request param={"key": ["a", 123, true], "key2": #[11, {"k": "v"}, [4, 5, 6], PublicKey(base58string)]}
```
Smart tokenization result:
```rust
["my_request", "param={\"key\": [\"a\", 123, true], \"key2\": #[11, {\"k\": \"v\"}, [4, 5, 6], PublicKey(base58string)]}"]
```
- Bracket-aware tokenizer preserves the entire complex literal as single token
- Parameter value validated directly using existing `Rule::test_literal_value`
- Real-time validation: ✓ Valid dictionary with nested arrays, sets, and PublicKey

**Built-in Command**:
```console
> ls zukyc
```
Parse result:
```rust
ConsoleCommand::BuiltIn {
    name: "ls".to_string(),
    args: vec!["zukyc".to_string()]
}
```

**Raw Podlang Execution**:
```console
> exec REQUEST(Equal(?pod["field"], "test value"))
```
Parse result:
```rust
ConsoleCommand::Exec {
    code: "REQUEST(Equal(?pod[\"field\"], \"test value\"))".to_string()
}
```

**Real-time Validation Examples**:
```console
> my_alias data=[1, 2,
  ▸ Parsing: Incomplete array literal...

> my_alias data=[1, 2, 3]  
  ✓ Valid array literal

> my_alias text="hello world" number=42
  ✓ String with spaces + integer (properly tokenized)

> my_alias nested={"users": [{"name": "Alice Smith"}, {"name": "Bob Jones"}]}
  ✓ Complex nested structure with spaces in values

> my_alias invalid=pod_id!
  ✗ Invalid Podlang literal: unexpected character '!'
```

**TOML Alias Example**:
```toml
[aliases]
complex_query = """
REQUEST(
    Equal(?permissions, ["read", "write"])
    Contains(?roles, #["admin", "verified"])
    Gt(?created_at, ?min_date)
)
"""
```

**Executed Command**:
```console
> complex_query permissions=["read","write","admin"] roles=#["admin","verified","user"] min_date=1640995200
```
Template binding produces:
```
REQUEST(
    Equal(["read","write","admin"], ["read", "write"])
    Contains(#["admin","verified","user"], #["admin", "verified"])
    Gt(1640995200, 1640995200)
)
```

### Dependencies

**Rust Backend Dependencies**:
- `toml` crate for configuration parsing
- `serde` for serialization (already in project)
- `chrono` for timestamp handling (already in project)  
- `tokio` for async operations (already in project)
- Additional Tauri filesystem permissions for config file access

**JavaScript Frontend Dependencies**:
- Minimal new dependencies (mostly display logic)
- Reuse existing UI components and theming

**Integration Points**:  
- Leverage existing Tauri RPC infrastructure
- Integrate with current theme and styling system
- Utilize existing POD state management via Rust backend
- Link to existing rich UI components (PodViewer, EditorResults)

### Backend-Heavy Architecture Design

**Core Principle**: The Rust backend handles all console logic, state, and processing. The JavaScript frontend is a thin display layer that renders messages and captures user input.

**Rust Backend Responsibilities**:
- Command parsing and validation
- Alias management and TOML configuration loading
- Wildcard parameter binding and substitution  
- Command history persistence
- Event bus and message routing
- Session state management (including current working folder)
- All POD operations (leveraging existing infrastructure)
- Current folder tracking and POD placement logic
- Timestamp generation and formatting
- Error handling and validation

**Frontend Responsibilities**:
- Display messages in scrollable output area
- Capture user input and send to backend
- Render timestamps and formatting
- Handle UI interactions (clicks, scrolling)
- Basic input validation (e.g., prevent empty commands)

**RPC Interface Design**:
```rust
// Primary command execution
#[tauri::command]
async fn console_execute_command(
    state: State<'_, Mutex<AppState>>,
    input: String,
) -> Result<ConsoleResponse, String>

// Get message history for display
#[tauri::command]  
async fn console_get_messages(
    state: State<'_, Mutex<AppState>>,
    since: Option<u64>, // timestamp filter
) -> Result<Vec<ConsoleMessage>, String>

// Get command history for autocomplete/navigation
#[tauri::command]
async fn console_get_command_history() -> Result<Vec<String>, String>

// Configuration management
#[tauri::command]
async fn console_reload_config() -> Result<(), String>

// Event subscription (for GUI events)
#[tauri::command]
async fn console_log_event(
    state: State<'_, Mutex<AppState>>,
    event: ConsoleEvent,
) -> Result<(), String>
```

**Message Flow**:
1. User types command in frontend input
2. Frontend sends command to `console_execute_command` RPC
3. Backend parses command, executes logic, generates response
4. Backend adds messages to session log with timestamps
5. Backend returns response with new messages
6. Frontend displays messages in output area
7. GUI operations emit events via `console_log_event` RPC

### Configuration File Management

**Two-File Configuration System**:

**1. Core Configuration (`podnet.toml`)**:
- Server URLs and endpoints
- System-level settings
- Environment-specific configuration
- Can be overridden via command line argument

**2. User Aliases (`aliases.toml`)**:  
- Command aliases and templates
- User-specific customizations
- Shared across different environments

**File Locations**:
- **Default**: `~/.local/share/com.0xparc.pod2/podnet.toml` and `aliases.toml`
- **Override**: `--config /path/to/podnet.toml` (aliases.toml in same directory)
- **Development**: Easy to switch environments while keeping user aliases

**Example podnet.toml**:
```toml
[servers]
identity_server = "https://identity.pod2.network"
document_server = "https://pod-server.ghost-spica.ts.net/server"
p2p_bootstrap_nodes = ["node1.pod2.network:7392", "node2.pod2.network:7392"]

[console]
show_timestamps = true
timestamp_format = "HH:MM:SS"
log_pod_operations = true
log_system_events = true
log_errors = true

[network]
p2p_port = 7392
max_connections = 50
```

**Example aliases.toml**:
```toml
[aliases]
zukyc = """
REQUEST(
    NotContains(?sanctions["sanctionList"], ?gov["idNumber"])
    Lt(?gov["dateOfBirth"], ?age_threshold)
    Equal(?pay["startDate"], ?start_date)
    Equal(?gov["socialSecurityNumber"], ?pay["socialSecurityNumber"])
)
"""

ethdos = """  
use _, _, _, eth_dos from ?batch

REQUEST(
    eth_dos(?src, ?dst, ?Distance)
)
"""
```

**Configuration Loading Logic**:
1. Load core config from `podnet.toml` (default location or `--config` path)
2. Look for `aliases.toml` in same directory as `podnet.toml`
3. Fall back to default locations if files don't exist
4. Auto-create with sensible defaults if missing

**Hot Reloading**: Watch both files for changes and reload automatically
**Validation**: Validate TOML syntax and Podlang templates on load
**Environment Switching**: `--config dev-podnet.toml` vs `--config prod-podnet.toml`

**Development Workflow Examples**:
```bash
# Development against local servers
pod2-client --config ./configs/dev-podnet.toml

# Staging environment  
pod2-client --config ./configs/staging-podnet.toml

# Production
pod2-client --config ./configs/prod-podnet.toml

# Each environment uses same ~/.local/share/com.0xparc.pod2/aliases.toml
```

**Configuration File Discovery**:
1. If `--config path/to/podnet.toml` specified:
   - Load core config from that path
   - Look for `aliases.toml` in same directory
   - Fall back to user aliases if not found
2. If no `--config` specified:
   - Load from default user directory
   - Create defaults if missing

### Error Handling

**Alias Errors**: Show clear error messages for invalid TOML or Podlang syntax
**Execution Errors**: Display solver errors with helpful context
**Parameter Errors**: Validate parameter names and types before execution
**File Errors**: Handle configuration file read/write permissions gracefully

## Usage Examples

### Basic Alias Usage
```
[14:32:15] > zukyc gov=pod_abc123 age_threshold=567890123
[14:32:15] Executing alias: zukyc
[14:32:15] ✓ Found POD gov: Government ID (John Doe)  
[14:32:15] ✓ Found POD sanctions: OFAC Sanctions List
[14:32:16] ✓ Execution successful
[14:32:16] Main POD: pod_new456 saved to current folder (zukyc/)
[14:32:16] Generated: 0xdef456789abcdef (click to view)
```

### POD Management Commands
```
[14:30:10] > pwd
[14:30:10] /default

[14:30:15] > ls
[14:30:15] default/   3 PODs
           zukyc/     3 PODs  
           ethdos/    2 PODs

[14:30:20] > cd zukyc
[14:30:20] Changed to folder: zukyc/

[14:30:25] > ls
[14:30:25] pod_abc123 Gov ID        (signed)  ZooGov
           pod_def456 Pay Stub      (signed)  ZooDeel
           pod_ghi789 Sanctions     (signed)  ZooOFAC

[14:30:30] > mv pod_abc123 archive
[14:30:30] Moved pod_abc123 to folder 'archive'

[14:30:35] > cat pod_abc123
[14:30:35] Signed POD pod_abc123:
           Type: Government ID
           Signer: ZooGov (0x1234...)
           Fields:
             idNumber: "123-45-6789"
             dateOfBirth: 567890123
             socialSecurityNumber: "987-65-4321"
```

### Command with Missing Parameters  
```  
> zukyc gov=pod_abc123
⚠ Unbound wildcard: ?age_threshold
Parameter binding:
  ?gov: ✓ pod_abc123
  ?age_threshold: ✗ unbound
  ?sanctions: auto-detected from available PODs
```

### Help and Discovery
```
> help
Built-in commands:
  ls [space]     - List PODs
  mv <id> <space> - Move POD to space
  rm <id>        - Delete POD
  cat <id>       - Show POD contents
  help [cmd]     - Show help
  
Aliases:
  zukyc          - Age and sanctions verification
  ethdos         - Ethereum degrees of separation
  
> alias zukyc
zukyc - Age and sanctions verification alias
Wildcards: ?gov ?sanctions ?age_threshold ?pay ?start_date
Example: zukyc gov=pod_123 age_threshold=946684800
```

### GUI Event Logging Examples
```
[14:31:45] 📥 POD imported via GUI: "government_id.pod" → zukyc
[14:31:50] ✏️  POD signed via GUI: "My KYC Document" → pod_new123
[14:32:05] 📁 POD moved via GUI: pod_old456 from default → archive
[14:32:10] > ls archive
[14:32:10] pod_abc123 Gov ID        (signed)  ZooGov
           pod_old456 User Document (signed)  MyKey
[14:32:30] ❌ Import failed: Invalid POD signature
```

### Mixed Command and Event Log Example
```
[14:30:00] 🔄 Console started, configuration loaded
[14:30:15] default/ > ls
[14:30:15] default/   2 PODs
           zukyc/     3 PODs
[14:31:22] 📥 POD imported via GUI: "payroll.pod" → default/
[14:31:30] default/ > cd zukyc
[14:31:45] zukyc/ > zukyc gov=pod_abc123 age_threshold=567890123
[14:31:45] Executing alias: zukyc
[14:31:46] ✓ Execution successful
[14:31:46] Main POD: pod_result123 saved to current folder (zukyc/)
[14:32:10] ✏️  POD signed via GUI: "Employment Verification" → default/ (pod_emp789)
[14:32:15] zukyc/ > mv pod_emp789 work
[14:32:15] Moved pod_emp789 from default/ to work/
[14:32:30] 🌐 P2P node started on port 7392
[14:32:45] zukyc/ > help
[14:32:45] Built-in commands:
           ls [folder]     - List PODs
           cd <folder>     - Change current folder
           mv <id> <folder> - Move POD to folder
           ...
```

## Event Logging and Timestamps

### Timestamp Display

All console messages should include timestamps for context and logging purposes:
- **Format**: `[HH:MM:SS]` (24-hour format)
- **Configurable Display**: Users can toggle timestamp visibility in console
- **Always Stored**: Timestamps always saved in session logs regardless of display setting
- **Timezone**: Local system timezone

### GUI Event Logging

The console should serve as a comprehensive activity log by capturing relevant events from the GUI:

**POD Operations**:
- `📥 POD imported via GUI: "filename.pod" → space_name`
- `✏️  POD signed via GUI: "Document Title" → pod_id`
- `🗑️ POD deleted via GUI: pod_id ("POD Name")`
- `📁 POD moved via GUI: pod_id from space_a → space_b`
- `📋 POD copied via GUI: pod_id → pod_new_id`

**System Events**:
- `🔑 Private key generated`
- `🌐 P2P node started on port 1234`
- `⚠️ Failed to connect to document server`
- `🔄 Configuration reloaded`

**Error Events**:
- `❌ Import failed: Invalid POD format`
- `❌ Signing failed: Missing private key`
- `❌ Network error: Connection timeout`

### Event Logging Configuration

Event logging configuration is now part of the core `podnet.toml` file:

```toml
[console]
# Timestamp display
show_timestamps = true
timestamp_format = "HH:MM:SS"  # or "HH:MM" or "full"

# Event logging from GUI
log_pod_operations = true
log_system_events = true  
log_errors = true
log_network_events = false
log_debug_events = false
```

This allows different environments to have different logging levels while user aliases remain consistent.

### Implementation Approach

**Event Bus System**:
- Create a console event bus that GUI operations can emit to
- Separate from internal logging/tracing system
- Only user-relevant events, not implementation details
- Events include severity level (info, warning, error)

**Event Types**:
```typescript
type ConsoleEvent = {
  timestamp: Date;
  type: 'command' | 'pod_operation' | 'system' | 'error';
  source: 'console' | 'gui' | 'system';
  message: string;
  data?: any; // Optional structured data
};
```

**Integration Points**:
- Tauri RPC commands emit events after successful operations
- Error handlers emit error events
- System startup/shutdown emit lifecycle events
- Import/export dialogs emit file operation events

## Current Folder Concept

The console maintains a **current working folder** concept similar to Unix shells. This determines where newly created PODs are automatically saved.

### Core Behavior:
- **Default folder**: Console starts in `/default` folder
- **Automatic POD placement**: Any command that generates a new POD saves it to the current folder
- **Explicit override**: Users can still move PODs to other folders with `mv` command
- **Shell-like navigation**: `cd <folder>` changes current folder, `pwd` shows current location

### Operations Affected by Current Folder:
- **Alias execution**: `zukyc gov=pod_123` → new MainPOD saved to current folder  
- **Pod signing via console**: Any new SignedPODs created through console commands
- **Pod copying**: `cp pod_123 new_name` → copy created in current folder
- **GUI integration**: When possible, GUI operations respect current console folder

### Folder Navigation:
```
> pwd                    # Shows current folder
/default

> cd work               # Change to 'work' folder  
Changed to folder: work/

> mkdir archive         # Create new folder
Created folder: archive/

> ls                    # List contents of current folder
pod_abc123 Document     (signed)  MyKey
pod_def456 Report       (main)    Generated
```

### State Persistence:
- Current folder persists across console sessions
- Stored in console session state (backend)
- GUI operations can query current folder for context

## Some general principles

- **Backend-Heavy Architecture**: Console logic (command parsing, alias management, history, event bus) lives primarily in the Rust Tauri backend. The JavaScript frontend is mostly for display and UI interaction.
- **Current Folder Paradigm**: All POD creation operations automatically use the current working folder, making the interface more intuitive and shell-like.
- Unicode and emojis are fine and should be used where appropriate, though not excessively so, and simpler unicode characters are preferable to complex emojis (e.g. ✓ is preferable to ✅)
- Events from GUI should be clearly distinguished (different color/icon)
- Console should feel like a comprehensive activity log
- Timestamps help users understand sequence of operations

## Future Enhancements

### Scripting and Automation
- Multi-command scripts stored in TOML
- Command chaining with pipes
- Conditional execution based on results
- Loop constructs for batch operations

### Advanced Configuration
- Multiple configuration files and profiles  
- Team/shared configuration repositories
- Alias libraries and package management
- Version control integration for configuration

### Integration Features  
- Export console sessions as plain text logs
- Copy/paste friendly output formatting
- Integration with external tools and APIs
- Session replay and command history export

