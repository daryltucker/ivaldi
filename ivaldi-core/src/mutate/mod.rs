//! # Mutation Module (The Hammer)
//!
//! ## PURPOSE
//! Handles destructive file operations (write, delete) with extreme prejudice (safety).
//!
//! ## PHILOSOPHY
//! - **Backup First**: Hashing and backing up before any change.
//! - **Smart Collision**: Never fail on existence; assume the agent meant well, save as sidecar.
//! - **Atomic**: Use temp files and rename to ensure partial writes never happen.
//! - **Journaled**: Every action is recorded.

pub mod types;
pub mod write;
pub mod edit;
pub mod json;
pub mod checkbox;
pub mod append_section;
pub mod rename_symbol;
mod backup;

pub use types::{WriteFileArgs, EditFileArgs, EditFilesArgs, RenameSymbolArgs, EditPreview};
pub use json::{EditJsonArgs, JsonOperation};
pub use checkbox::{ToggleCheckboxArgs, CheckboxState, CheckboxResult};
pub use append_section::{AppendToSectionArgs, InsertPosition, AppendResult};

use std::path::{Path, PathBuf};
use crate::undo::Journal;
use crate::IvaldiResponse;

pub struct Mutator;

impl Mutator {
    /// Write content to a file safely.
    pub fn write_file(root: &Path, args: WriteFileArgs, journal: &Journal) -> IvaldiResponse<PathBuf> {
        write::write_file(root, args, journal)
    }

    /// Edit a file surgically (The Scalpel).
    /// Returns PathBuf on success, or EditPreview if preview=true.
    pub async fn edit_file(root: &Path, args: EditFileArgs, journal: &Journal) -> IvaldiResponse<serde_json::Value> {
        // Call edit and convert result
        let result = edit::edit_file(root, args, journal).await;
        
        // Map the inner type to serde_json::Value
        IvaldiResponse {
            is_error: result.is_error,
            content: result.content.map(|c| serde_json::to_value(c).unwrap_or(serde_json::Value::Null)),
            ui_diffs: result.ui_diffs,
            error: result.error,
            advisory: result.advisory,
        }
    }

    /// Transactional multi-file edit.
    pub async fn edit_files(root: &Path, args: EditFilesArgs, journal: &Journal) -> IvaldiResponse<Vec<PathBuf>> {
        edit::edit_files(root, args, journal).await
    }

    /// Edit JSON files with semantic operations.
    pub fn edit_json(root: &Path, args: json::EditJsonArgs, journal: &Journal) -> IvaldiResponse<PathBuf> {
        json::edit_json(root, args, journal)
    }

    /// Toggle checkboxes in Markdown files with semantic operations.
    pub async fn toggle_checkbox(root: &Path, args: checkbox::ToggleCheckboxArgs, journal: &Journal) -> IvaldiResponse<checkbox::CheckboxResult> {
        checkbox::toggle_checkbox(root, args, journal).await
    }

    /// Append content to sections in documents with semantic operations.
    pub async fn append_to_section(root: &Path, args: append_section::AppendToSectionArgs, journal: &Journal) -> IvaldiResponse<append_section::AppendResult> {
        append_section::append_to_section(root, args, journal).await
    }

    /// Rename symbols across files with AST-aware matching.
    pub async fn rename_symbol(root: &Path, args: RenameSymbolArgs, journal: &Journal) -> IvaldiResponse<rename_symbol::RenameResult> {
        rename_symbol::rename_symbol(root, args, journal).await
    }
}