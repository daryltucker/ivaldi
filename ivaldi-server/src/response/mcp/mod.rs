//! Pure MCP (Model Context Protocol) response handling
//!
//! This module provides independent MCP response formatting and detection.
//! It implements the exact MCP JSON-RPC format from the working commit
//! 75267869cddfd85040bc62ac728200fc7d335819.
//!
//! ## Architecture
//!
//! - **format.rs**: Response formatting logic (JSON-RPC wrapping)
//! - **detect.rs**: MCP client detection
//! - **mod.rs**: Public API exports
//!
//! ## Design Principles
//!
//! - Zero shared code with other formats (OpenAI, OpenCode)
//! - Exact reproduction of working MCP implementation
//! - Protected from accidental changes during other format development

pub mod detect;
pub mod format;

// Re-export public API
pub use detect::detect_mcp_request;
pub use format::{format_error_content, format_success_content};
