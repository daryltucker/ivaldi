use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a unit of "Wisdom" - a recorded tool execution trace used for collective learning.
/// 
/// This structure is designed to be:
/// 1. **Privacy-Aware**: Arguments can be hashed or redacted.
/// 2. **Context-Rich**: Captures environment, outcome, and error details.
/// 3. **Future-Proof**: Includes versioning and flexible metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WisdomEntry {
    /// The specific tool invoked (e.g., "write_file", "run_command")
    pub tool_name: String,
    
    /// Unique hash of the arguments (SHA256).
    /// Used to identify "identical" operations without storing PII/Secrets.
    pub args_hash: String,
    
    /// Semantic context vector (optional).
    /// Can be derived from file path, description, or error message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Vec<f32>>,

    /// The outcome of the operation.
    pub outcome: Outcome,

    /// Execution duration in milliseconds.
    pub duration_ms: u64,

    /// Agent/System version.
    pub agent_version: String,
    
    /// ISO 8601 Timestamp.
    pub timestamp: String,
    
    /// Additional metadata (e.g., "file_extension": "rs", "retry_count": 2).
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Success,
    Failure {
        code: Option<String>,   // e.g. "EACCES", "Timeout"
        message: String,        // Redacted/Sanitized error message
    },
}

impl WisdomEntry {
    pub fn new(tool: &str, args_hash: &str, outcome: Outcome, duration: u64) -> Self {
        Self {
            tool_name: tool.to_string(),
            args_hash: args_hash.to_string(),
            context: None,
            outcome,
            duration_ms: duration,
            agent_version: env!("CARGO_PKG_VERSION").to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            metadata: HashMap::new(),
        }
    }

    /// Attach metadata
    pub fn with_meta(mut self, key: &str, value: impl Into<serde_json::Value>) -> Self {
        self.metadata.insert(key.to_string(), value.into());
        self
    }
}
