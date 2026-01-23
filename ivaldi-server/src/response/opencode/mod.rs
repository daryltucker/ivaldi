//! OpenCode Response Module
//!
//! Handles OpenCode-specific MCP client requirements including:
//! - Vercel AI SDK validation quirks
//! - Schema flattening for oneOf/allOf/anyOf compatibility
//! - OpenAI-compatible response formats

pub mod detect;
pub mod format;

// Re-export for convenience
pub use detect::detect_opencode_request;
pub use format::OpenCodeFormatter;