//! Response format modules
//!
//! This module contains different response format implementations
//! for various MCP clients with different expectations.

pub mod mcp;
pub mod openai;

pub use mcp::handle_mcp_response;
pub use openai::handle_openai_response;
