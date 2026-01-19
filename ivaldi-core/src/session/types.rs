use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use schemars::JsonSchema;

use super::conversation::ConversationContext;

/// Core session data structure
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Session {
    /// Unique identifier
    pub id: String,
    
    /// Working directory (where agent starts)
    pub root: PathBuf,
    
    /// Discovered project root (.git, Cargo.toml, etc.)
    pub project_root: Option<PathBuf>,
    
    /// Timestamps
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    
    /// Conversations within this session (chat-level contexts)
    #[serde(default)]
    pub conversations: HashMap<String, ConversationContext>,
    
    /// Additional metadata
    #[serde(default)]
    pub metadata: SessionMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema)]
pub struct SessionMetadata {
    /// Human-friendly name
    pub label: Option<String>,
    
    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
    
    /// Recent files for smart suggestions (LRU cache, max 100)
    #[serde(default)]
    pub recent_files: VecDeque<PathBuf>,
    
    /// Project-specific .aiignore patterns
    #[serde(default)]
    pub aiignore_patterns: Vec<String>,
}

/// Helper struct for serialization to sessions.toml
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct SessionStore {
    pub sessions: std::collections::HashMap<String, Session>,
}

// --- Argument Structs for MCP Tools ---

/// Arguments for the session_init tool
/// 
/// **Behavior**: Initializes a new session or switches to an existing one. Creates a dedicated state directory.
/// **Usage**: Call this when starting a new high-level task or moving to a different project area.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionInitArgs {
    /// Unique identifier
    pub id: String,
    
    /// Optional root directory. Defaults to CWD if not provided.
    pub root: Option<PathBuf>,
    
    /// Whether to create a new session if it doesn't exist. Default: true.
    #[serde(default = "default_true")]
    pub auto_create: bool,
}

/// Arguments for the session_list tool
/// 
/// **Behavior**: Lists all active and archived sessions.
/// **Usage**: Use to discover previous work contexts or resume an existing session.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionListArgs {}

/// Arguments for `session_get`
///
/// **Behavior**: Retrieves details for a specific session or the current one. **Returns**: Metadata, active headers, and stats. **Usage**: Use to verify current context or inspect another session's state.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionGetArgs {
    /// Optional ID. If omitted, returns current session.
    pub id: Option<String>,
}

/// Arguments for the session_update tool
/// 
/// **Behavior**: Updates metadata for the current session.
/// **Usage**: Use to add tags, change labels, or update status during a task.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionUpdateArgs {
    /// New human-readable label
    pub label: Option<String>,
    /// Tags to ADD to the session
    pub add_tags: Option<Vec<String>>,
    /// Tags to REMOVE from the session
    pub remove_tags: Option<Vec<String>>,
}

fn default_true() -> bool { true }
