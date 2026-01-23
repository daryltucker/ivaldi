//! Pure OpenAI Chat Completions response handling
//!
//! This module provides independent OpenAI response formatting and detection.
//! It implements the exact OpenAI Chat Completions API format.
//!
//! ## Architecture
//!
//! - **format.rs**: Chat completions response formatting logic
//! - **detect.rs**: OpenAI client detection
//! - **mod.rs**: Public API exports
//!
//! ## Design Principles
//!
//! - Zero shared code with other formats (MCP, OpenCode)
//! - Exact OpenAI API compatibility
//! - Protected from accidental changes during other format development

pub mod detect;
pub mod format;

// Re-export public API
pub use detect::detect_openai_request;
pub use format::{format_success_response, format_error_response};