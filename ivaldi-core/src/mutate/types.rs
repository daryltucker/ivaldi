use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Arguments for the write_file tool
///
/// **Behavior**: Writes full content to a file. Atomic, safe, and backed up.
/// **Smart Default**: If file exists and `overwrite=false`, content is **appended** to the end.
/// **Safety**:
/// - **Backup First**: Original content backed up to `.ivaldi/backups/`.
/// - **Advisory Richness**: Reveals original line/byte count and provides instructions on 'overwrite: true' when an implicit append occurs.
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
/// **Preview Mode**: Set `preview: true` to see the diff without applying changes.
///
/// **Usage**: Use to modify specific parts of a file without rewriting the whole content.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EditFileArgs {
    /// Path to the file to edit
    pub path: PathBuf,

    /// Replacement content
    pub replacement: String,

    /// vecq query selector (AST) (e.g., .functions[], .classes[], .structs[], .imports[], .comments[])
    pub query: Option<String>,

    /// Regex line matching selector
    pub grep: Option<String>,

    /// Start line (1-indexed)
    pub from_line: Option<usize>,

    /// End line (1-indexed)
    pub to_line: Option<usize>,

    /// Preview mode: if true, returns diff without applying changes
    #[serde(default)]
    pub preview: bool,
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

/// Arguments for the rename_symbol tool
///
/// **Behavior**: AST-aware symbol renaming across files with scope control.
/// **Symbol Types**: function, variable, class, struct, or "any" for all types.
/// **Scopes**: file (single file), directory (all files in directory), project (all project files).
/// **Safety**: Atomic multi-file operations with backup and rollback.
///
/// **Usage**: Use for safe refactoring of symbols across your codebase.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct RenameSymbolArgs {
    /// Path to the file containing the symbol to rename
    pub path: String,
    /// Current name of the symbol to rename
    pub old_name: String,
    /// New name for the symbol
    pub new_name: String,
    /// Optional: Specific type of symbol (function, variable, class, etc.)
    /// If None, will attempt to match any symbol type
    pub symbol_type: Option<String>,
    /// Optional: Scope limitation (file, directory, project)
    /// Defaults to "file" for safety
    pub scope: Option<String>,
}

fn default_false() -> bool {
    false
}

/// Result of a preview edit - shows what would change without applying
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EditPreview {
    /// Path to the file that would be edited
    pub path: PathBuf,
    /// The diff that would be applied
    pub diff: String,
    /// Original content preview (first 500 chars)
    pub original_preview: String,
    /// New content preview (first 500 chars)
    pub modified_preview: String,
    /// Number of lines changed
    pub lines_changed: usize,
    /// Heuristics that would be applied
    pub heuristics_triggered: Vec<String>,
}
