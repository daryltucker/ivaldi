//! MCP Response Formatting
//!
//! This module implements pure MCP JSON-RPC response formatting.
//! It reproduces the exact logic from working commit 75267869cddfd85040bc62ac728200fc7d335819.

use serde_json::{json, Value};

/// Format MCP success content for transport
///
/// Returns the content in the appropriate MCP format:
/// - For initialize: raw object with protocolVersion, serverInfo, etc.
/// - For tools/list: raw object with tools array
/// - For tool calls: {"isError": false, "content": [{"type": "text", "text": "..."}]}
///
/// The JSON-RPC wrapping is done by the transport layer (main.rs/server_http.rs).
pub fn format_success_content(content: Value) -> Value {
    // Check if this is an initialize response (has protocolVersion)
    if content.get("protocolVersion").is_some() {
        return content;
    }

    // Check if this is a tools/list response (has tools array)
    if content.get("tools").is_some() && content.get("tools").unwrap().is_array() {
        return content;
    }

    // For tool call responses, wrap in content array with isError
    let content_str = match content {
        Value::String(s) => s,
        _ => content.to_string(),
    };
    json!({
        "isError": false,
        "content": [{
            "type": "text",
            "text": content_str
        }]
    })
}

/// Format MCP error content for transport
///
/// Returns the error object that should go in the JSON-RPC result field.
/// Includes isError for backward compatibility.
/// The JSON-RPC wrapping is done by the transport layer (main.rs/server_http.rs).
pub fn format_error_content(code: String, message: String) -> Value {
    json!({
        "isError": true,
        "error": {
            "code": code,
            "message": message
        }
    })
}
