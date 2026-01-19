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
mod backup;

pub use types::{WriteFileArgs, EditFileArgs, EditFilesArgs};

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
    pub async fn edit_file(root: &Path, args: EditFileArgs, journal: &Journal) -> IvaldiResponse<PathBuf> {
        edit::edit_file(root, args, journal).await
    }

    /// Transactional multi-file edit.
    pub async fn edit_files(root: &Path, args: EditFilesArgs, journal: &Journal) -> IvaldiResponse<Vec<PathBuf>> {
        edit::edit_files(root, args, journal).await
    }
}