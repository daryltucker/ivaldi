use std::path::PathBuf;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Arguments for the write_file tool
/// 
/// **Behavior**: Writes full content to a file. Atomic, safe, and backed up.
/// **Smart Default**: If file exists and `overwrite=false`, content is **appended** to the end.
/// **Safety**:
/// - **Backup First**: Original content backed up to `.ivaldi/backups/`.
/// - **Advisory Richness**: Reveals original line/byte count when appending to help you decide if you intended to overwrite.
///   **Usage**: Use for creating new files or adding content. Use `overwrite=true` only when you are certain you want to replace everything.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WriteFileArgs {
    /// Path to the file to write
    pub path: PathBuf,

    /// Content to write
    pub content: String,

    /// Overwrite existing file (default: false)
    #[serde(default = "default_false")]
    pub overwrite: bool,

    /// Append to existing file (default: false)
    #[serde(default = "default_false")]
    pub append: bool,
}

/// Arguments for the edit_file tool
/// 
/// **Behavior**: Surgically modifies content using selectors.
/// **Selectors (Exactly one required)**:
/// - `query`: `vecq` query string for AST node matching.
/// - `grep`: Regex pattern for matching a single line.
/// - `from_line`/`to_line`: Exact 1-indexed line range.
///
/// **Safety**:
/// - **AST-First**: Nodes are more stable than lines for source code.
/// - **Pre-flight Logic**: Heuristics check for git-ignore and syntax errors.
///
/// **Usage**: Use to modify specific parts of a file without rewriting the whole content.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EditFileArgs {
    /// Path to the file to edit
    pub path: PathBuf,

    /// Replacement content
    pub replacement: String,

    /// vecq query selector (AST)
    pub query: Option<String>,

    /// Regex line matching selector
    pub grep: Option<String>,

    /// Start line (1-indexed)
    pub from_line: Option<usize>,

    /// End line (1-indexed)
    pub to_line: Option<usize>,

    /// Overwrite existing file (default: true)
    #[serde(default = "default_true")]
    pub overwrite: bool,
}

/// Arguments for the edit_files tool
/// 
/// **Behavior**: Transactional multi-file edit. Either all edits apply successfully, or the entire transaction is rolled back.
/// **Safety**: Atomic and journaled. Inherits safety from `edit_file`.
/// **Usage**: Use when a change spans multiple related files and must be applied consistently (e.g., refactoring a trait and all its usages).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EditFilesArgs {
    pub edits: Vec<EditFileArgs>,
}

fn default_false() -> bool { false }
fn default_true() -> bool { true }
