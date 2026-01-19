use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// Metadata provided by the IDE context for every tool call.
///
/// This structure represents the `_meta` field in the JSON-RPC params.
/// It allows the IDE to inject contextual information that is not part of the
/// tool's arguments, such as the current project root, conversation ID, or
/// active file.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IdeMetadata {
    /// The root directory of the project.
    #[serde(alias = "antigravity.google/project_root")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_root: Option<String>,

    /// The unique identifier for the current conversation.
    #[serde(alias = "antigravity.google/conversation_id")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    
    /// The active file being edited (if any).
    #[serde(alias = "antigravity.google/active_file")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_file: Option<String>,
    
    /// The line number of the cursor (if any).
    #[serde(alias = "antigravity.google/cursor_line")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor_line: Option<usize>,
}

/// Helper struct to generate the full `_meta` schema.
/// This matches how it appears in the JSON-RPC parameters.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolCallContext {
    #[serde(rename = "_meta")]
    pub meta: IdeMetadata,
}
