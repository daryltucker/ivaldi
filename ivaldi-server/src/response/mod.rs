//! Response Format Modules
//!
//! This module provides a modular response formatting system for the Ivaldi MCP server.
//! It supports multiple response formats to ensure compatibility with various clients:
//!
//! - **MCP**: Standard JSON-RPC format for proper MCP clients
//! - **OpenAI**: Chat completions API format for OpenAI-compatible clients
//!
//! ## Architecture
//!
//! ```text
//! response/
//! ├── mod.rs      # Registry + trait definitions
//! ├── types.rs    # Shared response types (ChatCompletion, McpResponse, etc.)
//! ├── mcp.rs      # MCP JSON-RPC formatter
//! └── openai.rs   # OpenAI chat completions formatter
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! let registry = response::get_registry();
//! let formatter = registry.get_formatter("mcp").unwrap();
//! let response = formatter.format_success(ivaldi_response)?;
//! ```

// ============================================================================
// MODULE DECLARATIONS
// ============================================================================

pub mod mcp;
pub mod openai;
