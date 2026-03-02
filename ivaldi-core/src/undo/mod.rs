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
use std::io::{Read, BufRead, BufReader, Write, Seek, SeekFrom};
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
    /// Optimisation: Seek to end and read backwards to avoid O(N) memory risk.
    pub fn head(&self) -> Result<Option<JournalEntry>> {
        let mut file = File::open(&self.path).context("Failed to open journal for reading")?;
        file.lock_shared().context("Failed to acquire shared lock")?;

        let metadata = file.metadata()?;
        let mut file_size = metadata.len();
        if file_size == 0 {
            let _ = file.unlock();
            return Ok(None);
        }

        let mut buffer = [0u8; 1024];
        let mut line_bytes = Vec::new();

        // 1. Check for trailing newline
        file.seek(SeekFrom::End(-1))?;
        let mut last_byte = [0u8; 1];
        file.read_exact(&mut last_byte)?;
        if last_byte[0] == b'\n' {
            file_size -= 1;
        }

        let mut pos = file_size;
        let mut found_line = false;

        while pos > 0 && !found_line {
            let read_size = std::cmp::min(pos, buffer.len() as u64);
            pos -= read_size;
            file.seek(SeekFrom::Start(pos))?;
            file.read_exact(&mut buffer[..read_size as usize])?;

            for i in (0..read_size as usize).rev() {
                if buffer[i] == b'\n' {
                    if !line_bytes.is_empty() {
                        found_line = true;
                        break;
                    }
                } else {
                    line_bytes.push(buffer[i]);
                }
            }
        }

        let result = if !line_bytes.is_empty() {
            line_bytes.reverse();
            let line = String::from_utf8(line_bytes).map_err(|e| anyhow::anyhow!("Invalid UTF-8 in journal: {}", e))?;
            let entry: JournalEntry = serde_json::from_str(&line)
                .with_context(|| format!("Failed to parse journal line: {}", line))?;
            Ok(Some(entry))
        } else {
            Ok(None)
        };

        let _ = file.unlock();
        result
    }

    /// Find the last entry that hasn't been undone yet.
    /// Uses backward scanning to avoid O(N) memory risk.
    pub fn find_last_undoable(&self) -> Result<Option<JournalEntry>> {
        let mut file = File::open(&self.path).context("Failed to open journal for reading")?;
        file.lock_shared().context("Failed to acquire shared lock")?;

        let metadata = file.metadata()?;
        let mut file_size = metadata.len();
        if file_size == 0 {
            let _ = file.unlock();
            return Ok(None);
        }

        let mut buffer = [0u8; 4096];
        let mut current_line = Vec::new();
        let mut undos_to_skip = 0;

        // Skip potential trailing newline
        file.seek(SeekFrom::End(-1))?;
        let mut b = [0u8; 1];
        file.read_exact(&mut b)?;
        if b[0] == b'\n' {
            file_size -= 1;
        }

        let mut pos = file_size;

        while pos > 0 {
            let chunk_size = std::cmp::min(pos, buffer.len() as u64);
            pos -= chunk_size;
            file.seek(SeekFrom::Start(pos))?;
            file.read_exact(&mut buffer[..chunk_size as usize])?;

            for i in (0..chunk_size as usize).rev() {
                if buffer[i] == b'\n' {
                    if !current_line.is_empty() {
                        current_line.reverse();
                        let line = String::from_utf8(current_line.clone()).map_err(|e| anyhow::anyhow!("Invalid UTF-8: {}", e))?;
                        let entry: JournalEntry = serde_json::from_str(&line)?;
                        current_line.clear();

                        if entry.action == ActionType::Undo {
                            undos_to_skip += 1;
                        } else if undos_to_skip > 0 {
                            undos_to_skip -= 1;
                        } else {
                            let _ = file.unlock();
                            return Ok(Some(entry));
                        }
                    }
                } else {
                    current_line.push(buffer[i]);
                }
            }
        }

        // Handle the first line
        if !current_line.is_empty() {
            current_line.reverse();
            let line = String::from_utf8(current_line).map_err(|e| anyhow::anyhow!("Invalid UTF-8: {}", e))?;
            let entry: JournalEntry = serde_json::from_str(&line)?;
            if entry.action != ActionType::Undo && undos_to_skip == 0 {
                let _ = file.unlock();
                return Ok(Some(entry));
            }
        }

        let _ = file.unlock();
        Ok(None)
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
        let last_entry = match journal.find_last_undoable() {
            Ok(Some(e)) => e,
            Ok(None) => return IvaldiResponse::from_error(IvaldiError::Internal("Nothing to undo".into())),
            Err(e) => return IvaldiResponse::from_error(IvaldiError::Journal(e.to_string())),
        };

        let target_path = &last_entry.path;

        if target_path.exists() && let Some(expected_hash) = &last_entry.checksum_after {
            let current_hash = match calculate_sha256(target_path) {
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
