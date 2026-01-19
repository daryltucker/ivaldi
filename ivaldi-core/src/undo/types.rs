//! # Journal Types
//!
//! ## PURPOSE
//! Defines the immutable events recorded in the append-only journal.
//!
//! ## PHILOSOPHY
//! - **Immutable**: Once written, an entry is never changed.
//! - **Append-Only**: History only moves forward.
//! - **Complete**: Contains enough info to reverse the operation.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Unique identifier for a journal entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OpId(pub Uuid);

impl OpId {
    pub fn new() -> Self {
        OpId(Uuid::new_v4())
    }
}

impl Default for OpId {
    fn default() -> Self {
        Self::new()
    }
}

/// The type of operation performed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    /// File was created
    Create,
    /// File was modified (overwritten)
    Update,
    /// File was deleted
    Delete,
    /// File was moved/renamed
    Move,
    /// An undo operation (restoring a previous state)
    Undo,
}

/// A single entry in the operation journal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    /// Unique ID for this operation
    pub id: OpId,
    
    /// When it happened
    pub timestamp: DateTime<Utc>,
    
    /// What kind of action
    pub action: ActionType,
    
    /// The primary file affected
    pub path: PathBuf,
    
    /// If moved, the destination (or source? usually dest for the record)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination: Option<PathBuf>,
    
    /// Checksum of file BEFORE operation (SHA-256 hex)
    /// None if file didn't exist (Create)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum_before: Option<String>,
    
    /// Checksum of file AFTER operation
    /// None if file deleted
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum_after: Option<String>,
    
    /// Path to the backup file containing the "before" state
    /// Critical for Undo.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_ref: Option<PathBuf>,
    
    /// User/Agent who performed the action (optional context)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
}

impl JournalEntry {
    pub fn new(action: ActionType, path: PathBuf) -> Self {
        Self {
            id: OpId::new(),
            timestamp: Utc::now(),
            action,
            path,
            destination: None,
            checksum_before: None,
            checksum_after: None,
            backup_ref: None,
            actor: None,
        }
    }
}
