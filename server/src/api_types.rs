use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use pod2::frontend::{SerializedMainPod, SerializedSignedPod};

// --- General API Data Structures ---

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct SpaceInfo {
    pub id: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
#[serde(tag = "pod_data_variant", content = "pod_data_payload")]
pub enum PodData {
    Signed(SerializedSignedPod),
    Main(SerializedMainPod),
}

impl PodData {
    /// Returns a string representation of the pod data variant.
    pub fn type_str(&self) -> &'static str {
        match self {
            PodData::Signed(_) => "signed",
            PodData::Main(_) => "main",
        }
    }
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct PodInfo {
    pub id: String,
    pub pod_type: String,
    pub data: PodData,
    pub label: Option<String>,
    pub created_at: String,
    pub space: String,
}

// --- Validation Structures ---

#[derive(Deserialize, Debug)]
pub struct ValidateCodeRequest {
    pub code: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct Diagnostic {
    pub message: String,
    pub severity: DiagnosticSeverity,
    pub start_line: usize,   // 1-based
    pub start_column: usize, // 1-based
    pub end_line: usize,     // 1-based
    pub end_column: usize,   // 1-based
}

#[derive(Serialize, Debug, Clone)]
pub enum DiagnosticSeverity {
    Error,
    // In the future, we can add: Warning, Information, Hint
}

#[derive(Serialize, Debug, Clone)]
pub struct ValidateCodeResponse {
    pub diagnostics: Vec<Diagnostic>,
}

// --- Execution Structures ---

#[derive(Deserialize, Debug)]
pub struct ExecuteMvpRequest {
    pub code: String,
}

fn mock_default() -> bool {
    false
}

#[derive(Deserialize, Debug)]
pub struct ExecuteCodeRequest {
    pub code: String,
    pub space_id: String,
    #[serde(default = "mock_default")]
    pub mock: bool,
}

// For a successful execution, the response body will be `Json<serde_json::Value>`
// where the `serde_json::Value` is a JSON string containing the execution result.
// Errors during execution (including validation errors if code is invalid)
// will be handled by a new `PlaygroundApiError` type returned by the handlers.
