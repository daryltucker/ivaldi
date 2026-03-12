//! MCP Response Formatting
//!
//! This module implements pure MCP JSON-RPC response formatting.
//! It reproduces the exact logic from working commit 75267869cddfd85040bc62ac728200fc7d335819.

use serde_json::{json, Value};

use ivaldi_core::IvaldiResponse;

/// Format a full IvaldiResponse for an MCP tool call
/// 
/// Preserves advisories by adding them as additional content items.
pub fn format_tool_response(response: IvaldiResponse<Value>) -> Value {
    // 0. Fast-path for protocol responses (initialize, tools/list)
    if let Some(content) = &response.content {
        if content.get("protocolVersion").is_some() || 
           (content.get("tools").is_some() && content.get("tools").unwrap().is_array()) {
            return content.clone();
        }
    }

    let mut content_items = Vec::new();
    
    // 1. Add Primary Content
    if let Some(content) = response.content {
        // Smart Content Extraction
        let content_str = if let Some(obj) = content.as_object() {
            if let Some(c) = obj.get("content").and_then(|v| v.as_str()) {
                c.to_string()
            } else {
                serde_json::to_string_pretty(&content).unwrap_or_else(|_| content.to_string())
            }
        } else if let Some(s) = content.as_str() {
            s.to_string()
        } else {
            content.to_string()
        };
        
        content_items.push(json!({
            "type": "text",
            "text": content_str,
            "annotations": {
                "audience": ["assistant"]
            }
        }));
    } else if response.is_error {
        // Add Error Message as first content item
        if let Some(err) = &response.error {
            content_items.push(json!({
                "type": "text",
                "text": format!("Error [{}]: {}", err.code, err.message),
                "annotations": {
                    "audience": ["assistant"]
                }
            }));
        }
    }
    
    // 1.5 Add Visual UI Diffs (For humans only)
    for diff in response.ui_diffs {
        content_items.push(json!({
            "type": "text",
            "text": diff,
            "annotations": {
                "audience": ["user"]
            }
        }));
    }
    
    // 2. Add Advisories
    for adv in response.advisory {
        let text = if adv.content.is_string() {
            adv.content.as_str().unwrap().to_string()
        } else {
            serde_json::to_string_pretty(&adv.content).unwrap_or_else(|_| adv.content.to_string())
        };
        
        use ivaldi_core::advisory::AdvisoryLevel;
        let level_prefix = match adv.level {
            AdvisoryLevel::Warn => "⚠️ ADVISORY (Warning): ",
            AdvisoryLevel::Info => "ℹ️ ADVISORY (Info): ",
            AdvisoryLevel::Suggest => "💡 ADVISORY (Suggestion): ",
        };
        
        content_items.push(json!({
            "type": "text",
            "text": format!("{}{}", level_prefix, text)
        }));
    }

    let mut result = json!({
        "isError": response.is_error,
        "content": content_items
    });
    
    // Add structured error for machine-reading if present
    if let Some(err) = response.error {
        result["error"] = json!({
            "code": err.code,
            "message": err.message
        });
    }
    
    result
}

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

    // Fallback for simple values (legacy)
    let content_str = if let Some(obj) = content.as_object() {
        if let Some(c) = obj.get("content").and_then(|v| v.as_str()) {
            c.to_string()
        } else {
            serde_json::to_string_pretty(&content).unwrap_or_else(|_| content.to_string())
        }
    } else if let Some(s) = content.as_str() {
        s.to_string()
    } else {
        content.to_string()
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
/// Includes isError and a content array for spec compliance.
pub fn format_error_content(code: String, message: String) -> Value {
    json!({
        "isError": true,
        "content": [{
            "type": "text",
            "text": format!("Error [{}]: {}", code, message)
        }],
        "error": {
            "code": code,
            "message": message
        }
    })
}
