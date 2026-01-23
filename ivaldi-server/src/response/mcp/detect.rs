//! MCP Client Detection
//!
//! This module detects whether a request is from an MCP client.
//! Since MCP is the default format, detection is primarily about
//! ruling out other client types (OpenAI, OpenCode).

use serde_json::Value;

/// Detect if a request is from an MCP client
///
/// MCP is the default format, so this returns true unless the request
/// clearly indicates it's from another client type (OpenAI, OpenCode).
///
/// Current detection rules:
/// - Not OpenAI (no _meta.client_type == "openai")
/// - Not OpenCode (no opencode indicators)
/// - Default to MCP for everything else
pub fn detect_mcp_request(request: &Value) -> bool {
    // Check for OpenAI indicators
    if let Some(meta) = request.get("_meta") {
        if let Some(client_type) = meta.get("client_type").and_then(|v| v.as_str()) {
            if client_type.to_lowercase().contains("openai") {
                return false;
            }
        }
    }

    // Check for OpenCode indicators (simplified version)
    if let Some(meta) = request.get("_meta") {
        if let Some(client_type) = meta.get("client_type").and_then(|v| v.as_str()) {
            if client_type.to_lowercase().contains("opencode") {
                return false;
            }
        }
        if let Some(client_name) = meta.get("client_name").and_then(|v| v.as_str()) {
            if client_name.to_lowercase().contains("opencode") {
                return false;
            }
        }
    }

    // Default to MCP
    true
}
