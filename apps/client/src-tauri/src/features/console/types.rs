use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Core console data types and structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleMessage {
    pub id: u64,
    pub timestamp: DateTime<Utc>,
    pub message_type: MessageType,
    pub content: String,
    pub source: MessageSource,
    pub current_folder: String, // Folder context at time of message
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    Command,       // User-entered commands
    CommandResult, // Command output/results
    GuiEvent,      // GUI operations (imports, signing, etc.)
    SystemEvent,   // Startup, shutdown, errors
    Error,         // Error messages
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageSource {
    Console, // From console command
    Gui,     // From GUI operation
    System,  // System-generated
}

/// Console command types after parsing
#[derive(Debug, Clone, PartialEq)]
pub enum ConsoleCommand {
    BuiltIn {
        name: String,
        args: Vec<String>,
    },
    Alias {
        name: String,
        params: HashMap<String, String>,
    },
    Exec {
        code: String,
    },
}

/// Response from console command execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsoleResponse {
    Success {
        messages: Vec<ConsoleMessage>,
        current_folder: String,
    },
    Error {
        error: String,
        current_folder: String,
    },
}

/// Console state information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleState {
    pub current_folder: String,
    pub total_message_count: u64,
    pub aliases_loaded: bool,
    pub config_file_status: String,
}

/// Paginated console messages for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleMessagePage {
    pub messages: Vec<ConsoleMessage>,
    pub total_count: u64,
    pub has_more: bool,
}

/// Console event for GUI integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: String, // "pod_operation", "system", "error"
    pub source: MessageSource,
    pub message: String,
    pub data: Option<serde_json::Value>, // Optional structured data
}

/// Validation result for real-time parameter feedback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationResult {
    Valid,
    Invalid {
        error: String,
        suggestion: Option<String>,
    },
}
