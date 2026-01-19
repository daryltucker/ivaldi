use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use std::path::PathBuf;

/// Arguments for the git_read tool
/// 
/// **Behavior**: Provides read-only access to Git history. Supports blame, log, diff, and search operations.
/// 
/// **Actions**:
/// - `blame`: Identifies who changed which lines in a file.
/// - `log`: Lists commit history, optionally filtered by path or time.
/// - `diff`: Returns the changes between branches, tags, or commits.
/// - `search`: Searches commit messages or content using regex.
/// - `raw`: Executes arbitrary git commands (use with caution).
/// 
/// **Safety**: Read-only. Does not modify the repository state.
/// 
/// **Usage**: Use to understand the evolution of code, find when a bug was introduced, or compare implementations across versions.
#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone)]
pub struct GitReadArgs {
    #[serde(flatten)]
    pub action: GitAction,
    
    /// Optional: Override project root for git operations.
    /// If omitted, uses session's project_root from MCP initialize.
    pub project_root: Option<PathBuf>,
}

/// Git actions
#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum GitAction {
    #[serde(rename = "blame")]
    Blame {
        path: PathBuf,
        #[serde(default)]
        lines: Option<Vec<usize>>,
    },
    #[serde(rename = "log")]
    Log {
        path: Option<PathBuf>,
        #[serde(default = "default_limit")]
        limit: usize,
        since: Option<String>,  // "1w", "1d", or ISO timestamp
    },
    #[serde(rename = "diff")]
    Diff {
        from: String,  // Branch, tag, or commit
        #[serde(default = "default_head")]
        to: String,
        path: Option<PathBuf>,
        #[serde(default)]
        stat_only: bool,
    },
    #[serde(rename = "search")]
    Search {
        query: String,  // Regex to search for
        path: Option<PathBuf>,
        #[serde(default = "default_limit")]
        limit: usize,
    },
    #[serde(rename = "raw")]
    Raw {
        args: Vec<String>,
    },
}

fn default_limit() -> usize { 50 }
fn default_head() -> String { "HEAD".to_string() }
