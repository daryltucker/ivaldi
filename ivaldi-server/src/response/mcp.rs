//! MCP (Model Context Protocol) response formatting
//!
//! This module handles responses in the standard MCP JSON-RPC format.

use ivaldi_core::IvaldiResponse;
use serde_json::{json, Value};

/// Format response in MCP standard (errors in JSON-RPC error field)
/// This is the standard for MCP clients like opencode, VS Code extensions
/// - Success: {"jsonrpc": "2.0", "id": 1, "result": {"content": [...]}}
/// - Error: {"jsonrpc": "2.0", "id": 1, "error": {"code": -32601, "message": "..."}}
pub fn handle_mcp_response(response: IvaldiResponse<Value>) -> Result<Value, String> {
    // If it's an error, return as JSON-RPC error (will be handled by main loop)
    if response.is_error {
        return Err(response.error.as_ref().unwrap().message.clone());
    }

    // Success: format as MCP result with content array
    let mut result = json!({ "content": [] });
    if let Some(content) = response.content {
        result["content"] = json!([
            { "type": "text", "text": content }
        ]);
    }

    Ok(result)
}
