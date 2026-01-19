//! # ivaldi-core
//!
//! Core library for ivaldi-mcp: precision file operations for AI Agents.
//!
//! ## PURPOSE
//!
//! This crate contains the business logic, types, and traits that power ivaldi.
//! It is interface-agnostic - both CLI and MCP server depend on this crate.
//!
//! ## ARCHITECTURE
//!
//! ```text
//! ivaldi-core/
//! ├── response.rs   → IvaldiResponse<T>, AdvisoryMessage
//! ├── advisory.rs   → Advisory message construction helpers
//! ├── file_ops.rs   → File operations (find, read, write, edit)
//! ├── ast_edit.rs   → AST-based editing via vecq (future)
//! ├── undo.rs       → Operation journal and undo stack
//! └── aiignore.rs   → .aiignore parsing
//! ```
//!
//! ## KEY TYPES
//!
//! - [`IvaldiResponse<T>`] - All operations return this wrapper
//! - [`AdvisoryMessage`] - Third channel messages (tool/server/adt)
//! - [`FileTarget`] - Path or AST node reference
//! - [`Operation`] - Journaled operation for undo
//!
//! ## THE THIRD CHANNEL (stdinfo)
//!
//! Every response includes an optional `advisory` field:
//!
//! ```json
//! {
//!   "status": "success",
//!   "result": { ... },
//!   "advisory": [
//!     { "source": "adt", "level": "suggest", "message": "..." }
//!   ]
//! }
//! ```
//!
//! This enables coaching without failure - agents receive wisdom even on success.
//!
//! ## PHILOSOPHY
//!
//! 1. **All operations return IvaldiResponse** - never raw data
//! 2. **Advisory channel always available** - populated when relevant
//! 3. **Pre-flight validation** - check before write, not after
//! 4. **Operations are journaled** - undo stack for recovery
//! 5. **AST-first editing** - node references, not line numbers
//!
//! See: `docs/PHILOSOPHY.md` for full philosophy documentation.

pub mod response;
pub mod advisory;
pub mod error;
pub mod navigate;   // Phase 1 (Radar)
pub mod observe;    // Phase 1 (Telescope)
pub mod list;       // Phase 1 (Sensors)
// pub mod file_ops;   // Phase 1 (Legacy/Refactor target)
pub mod ast_edit;   // Phase 2
pub mod undo;       // Phase 1.4 (Journal)
pub mod lifecycle; // Added lifecycle module
pub mod mutate;     // Phase 1.4 (The Hammer)
pub mod heuristics; // The Opinion Module
pub mod util;
pub mod wisdom;     // Phase 4 (The Wisdom Layer)
pub mod config;     // Global Configuration
pub mod session;    // Phase 4 (Sessions)
pub mod meta;       // Phase 4 (IDE Metadata)
// pub mod aiignore;   // Phase 1

pub use response::{IvaldiResponse, ResponseStatus};
pub use advisory::{AdvisoryMessage, AdvisoryLevel, AdvisorySource};
pub use meta::IdeMetadata;
pub mod policy;
pub mod execution;
