//! # Undo Journal Module
//!
//! ## PURPOSE
//! Manages the append-only log of operations for the Undo Stack.
//!
//! ## PHILOSOPHY
//! - **Concurrency Safe**: Uses file locking (`fs2`) to safely handle multiple agents/processes.
//! - **Append Only**: We never rewrite history, we only add to it.
//! - **JSONL**: Simple, corrupt-resistant, streamable format.

pub mod types;

use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::io::{BufRead, BufReader, Write};
use fs2::FileExt;
use types::JournalEntry;
use anyhow::{Context, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Arguments for the undo tool
/// 
/// **Behavior**: Reverts the last operation performed in the project scope.
/// **Mechanism**: Reads journal, finds last backup, and restores it.
/// **Safety**:
/// - **Checksum Verification**: Ensures file hasn't been modified externally since the last operation.
/// - **Journaling**: The undo itself is logged as a new operation.
///   **Usage**: Call immediately if a `write_file` or `edit_file` had unintended consequences.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UndoArgs {
    /// Optional path to anchor project root discovery (default: ".")
    #[serde(default = "default_undo_path")]
    pub path: PathBuf,
}

fn default_undo_path() -> PathBuf { PathBuf::from(".") }

/// The Journal manager.
pub struct Journal {
    path: PathBuf,
}

impl Journal {
    /// Open a journal at the specified path.
    /// Creates the file (and parent dirs) if it doesn't exist.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        // Ensure file exists
        if !path.exists() {
            File::create(&path)?;
        }
        
        Ok(Self { path })
    }

    /// Append an entry to the journal safely.
    /// Locks the file (exclusive), appends, and unlocks.
    pub fn append(&self, entry: &JournalEntry) -> Result<()> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .context("Failed to open journal for appending")?;

        // Critical Section: Lock
        file.lock_exclusive().context("Failed to lock journal")?;

        let write_result = (|| -> Result<()> {
            let mut writer = std::io::BufWriter::new(&file);
            let json = serde_json::to_string(&entry)?;
            writeln!(writer, "{}", json)?;
            writer.flush()?;
            Ok(())
        })();

        // Unlock regardless of write result
        // Note: File lock is released when file is dropped/closed, but explicit unlock is good hygiene.
        let _ = file.unlock(); 

        write_result.context("Failed to append entry to journal")
    }

    /// Read all entries from the journal.
    /// Locks the file (shared) for reading.
    pub fn read_all(&self) -> Result<Vec<JournalEntry>> {
        let file = File::open(&self.path).context("Failed to open journal for reading")?;
        
        // Shared lock allows multiple readers but no writers
        file.lock_shared().context("Failed to acquire shared lock on journal")?;

        let reader = BufReader::new(&file);
        let mut entries = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() { continue; }
            let entry: JournalEntry = serde_json::from_str(&line)
                .with_context(|| format!("Failed to parse journal line: {}", line))?;
            entries.push(entry);
        }

        let _ = file.unlock();
        Ok(entries)
    }
    
    /// Get the latest entry (tail).
    /// Optimisation: We could seek to end and read backwards, but for V1 just read_all.
    pub fn head(&self) -> Result<Option<JournalEntry>> {
        let entries = self.read_all()?;
        Ok(entries.last().cloned())
    }
}

/// The Undo Manager
pub struct Undoer;

use crate::IvaldiResponse;
use crate::undo::types::ActionType;

impl Undoer {
    /// Undo the last operation in the journal.
    pub fn undo_last(_root: &Path, journal: &Journal) -> IvaldiResponse<PathBuf> {
        use crate::error::IvaldiError;
        let entries = match journal.read_all() {
            Ok(e) => e,
            Err(e) => return IvaldiResponse::from_error(IvaldiError::Journal(e.to_string())),
        };

        if entries.is_empty() {
            return IvaldiResponse::from_error(IvaldiError::Internal("Journal is empty".into()));
        }

        let mut active_stack = Vec::new();
        for (idx, entry) in entries.iter().enumerate() {
            if entry.action == ActionType::Undo {
                active_stack.pop();
            } else {
                active_stack.push(idx);
            }
        }

        let target_idx = match active_stack.last() {
            Some(&idx) => idx,
            None => return IvaldiResponse::from_error(IvaldiError::Internal("Nothing to undo".into())),
        };

        let last_entry = &entries[target_idx];
        let target_path = &last_entry.path;

        if last_entry.path.exists() && let Some(expected_hash) = &last_entry.checksum_after {
            let current_hash = match calculate_sha256(&last_entry.path) {
                Ok(h) => h,
                Err(e) => return IvaldiResponse::from_error(IvaldiError::Internal(e.to_string())),
            };

            if current_hash != *expected_hash {
                return IvaldiResponse::from_error(IvaldiError::Undo("File changed since last operation. Aborting undo.".into()));
            }
        }

        if let Some(backup_ref) = &last_entry.backup_ref {
            if !backup_ref.exists() {
                return IvaldiResponse::from_error(IvaldiError::FileNotFound(backup_ref.clone()));
            }
            if let Err(e) = fs::copy(backup_ref, target_path) {
                return IvaldiResponse::from_error(IvaldiError::Io(e));
            }
        } else if target_path.exists() && let Err(e) = fs::remove_file(target_path) {
            return IvaldiResponse::from_error(IvaldiError::Io(e));
        }

        let mut entry = JournalEntry::new(ActionType::Undo, target_path.clone());
        entry.actor = Some("undoer".into());

        if let Err(e) = journal.append(&entry) {
            return IvaldiResponse::from_error(IvaldiError::Journal(format!("Undo succeeded but logging failed: {}", e)));
        }

        IvaldiResponse::success(target_path.clone())
            .with_advisory(crate::AdvisoryMessage::tool_info(format!("Undid action {} ({:?})", last_entry.id.0, last_entry.action)))
    }
}

fn calculate_sha256(path: &Path) -> Result<String> {
    use sha2::{Sha256, Digest};
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}
